use anyhow::{bail, Context, Result};
use log::{debug, error};
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::fs::{self, read_dir, DirEntry};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct DiskMonitor {
    registry: Registry,
    disk_status: GaugeVec,
    sysfs: PathBuf,
    textfile: PathBuf,
}

impl DiskMonitor {
    pub fn new(sysfs: PathBuf, textfile: PathBuf) -> Result<Self> {
        let registry = Registry::new();
        let disk_status = GaugeVec::new(
            Opts::new("disk_status", "Status of the disk (1=active, 0=standby)"),
            &["disk"],
        )?;
        registry
            .register(Box::new(disk_status.clone()))
            .context("Failed to create prometheus registry")?;
        Ok(DiskMonitor {
            registry,
            disk_status,
            sysfs,
            textfile,
        })
    }

    pub fn update_metrics(&self, disk_query: &impl DiskStatus) -> Result<()> {
        let all_disks = get_all_disks(&self.sysfs)?;
        debug!("Loaded all disks: {:?}", all_disks);
        for disk in all_disks {
            if let Some(status) = disk_query
                .get_disk_status(&disk)
                .context("failed to get disk status")?
            {
                self.disk_status.with_label_values(&[&disk]).set(status);
            }
        }
        self.write_textfile()
    }

    fn write_textfile(&self) -> Result<()> {
        let textfile = fs::File::create(&self.textfile).with_context(|| {
            format!(
                "Failed to create textfile: {}",
                &self.textfile.to_string_lossy()
            )
        })?;
        let mut textfile = BufWriter::new(textfile);
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder
            .encode(&metric_families, &mut textfile)
            .context("Failed to encode metrics into textfile")?;
        Ok(())
    }
}

pub trait DiskStatus {
    fn get_disk_status(&self, disk: &str) -> Result<Option<f64>>;
}

pub struct Hdparm {
    pub path: String,
}
impl DiskStatus for Hdparm {
    fn get_disk_status(&self, disk: &str) -> Result<Option<f64>> {
        let output = Command::new(&self.path)
            .arg("-C")
            .arg(disk)
            .output()
            .context("Failed to execute hdparm")?;

        if !output.status.success() {
            error!("hdparm failed to execute: {:?}", output);
            bail!("hdparm execution error: {:?}", output);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!(
            "hdparm finished with exit_code: {}, stderr: '{}', stdout: '{}'",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            stdout
        );
        if stdout.contains("standby") {
            Ok(Some(0.0))
        } else if stdout.contains("active/idle") {
            Ok(Some(1.0))
        } else {
            Ok(None)
        }
    }
}

fn get_all_disks(sysfs: &Path) -> Result<Vec<String>> {
    let block_dir = sysfs.join("block");
    let dir_entries: Result<Vec<DirEntry>, _> = read_dir(block_dir)
        .context("Failed to list sysfs/block")?
        .collect();
    let result: Vec<String> = dir_entries?
        .into_iter()
        .map(|d| String::from(d.file_name().to_string_lossy()))
        .filter(|d| d.starts_with("sd"))
        .map(|d| format!("/dev/{d}"))
        .collect();
    Ok(result)
}

#[cfg(test)]
mod test {
    use std::fs::{self, create_dir};

    use tempfile::TempDir;

    use super::*;

    struct FakeHdparm {}
    impl DiskStatus for FakeHdparm {
        fn get_disk_status(&self, _disk: &str) -> Result<Option<f64>> {
            Ok(Some(0.0))
        }
    }

    #[test]
    fn it_works() {
        // prepare test
        let sysfs = TempDir::new().unwrap();
        let block = sysfs.path().join("block");
        create_dir(&block).unwrap();
        fs::write(block.join("sda"), "").unwrap();
        let textfile_collector = TempDir::new().unwrap();
        let disk_status = textfile_collector.path().join("disk_status.prom");
        let disk_query = FakeHdparm {};
        let monitor =
            DiskMonitor::new(sysfs.path().to_path_buf(), disk_status.to_path_buf()).unwrap();

        // run a single cycle
        monitor.update_metrics(&disk_query).unwrap();

        // compare results
        let disk_metrics = fs::read_to_string(&disk_status).unwrap();
        let expected = String::from(
            "# HELP disk_status Status of the disk (1=active, 0=standby)
# TYPE disk_status gauge
disk_status{disk=\"/dev/sda\"} 0\n",
        );
        assert_eq!(disk_metrics, expected);
    }
}
