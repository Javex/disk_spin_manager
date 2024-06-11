use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::fs::{self, read_dir};
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
    pub fn new(sysfs: PathBuf, textfile: PathBuf) -> Self {
        let registry = Registry::new();
        let disk_status = GaugeVec::new(
            Opts::new("disk_status", "Status of the disk (1=active, 0=standby)"),
            &["disk"],
        )
        .unwrap();
        registry.register(Box::new(disk_status.clone())).unwrap();
        DiskMonitor {
            registry,
            disk_status,
            sysfs,
            textfile,
        }
    }

    pub fn update_metrics(&self, disk_query: &impl DiskStatus) {
        let all_disks = get_all_disks(&self.sysfs);
        println!("all_disks: {:?}", all_disks);
        for disk in all_disks {
            if let Some(status) = disk_query.get_disk_status(&disk) {
                self.disk_status.with_label_values(&[&disk]).set(status);
            }
        }
        self.write_textfile()
    }

    fn write_textfile(&self) {
        let textfile = fs::File::create(&self.textfile).unwrap();
        let mut textfile = BufWriter::new(textfile);
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode(&metric_families, &mut textfile).unwrap();
    }
}

pub trait DiskStatus {
    fn get_disk_status(&self, disk: &str) -> Option<f64> {
        let output = Command::new("hdparm")
            .arg("-C")
            .arg(disk)
            .output()
            .expect("Failed to execute hdparm");

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("standby") {
            Some(0.0)
        } else if stdout.contains("active/idle") {
            Some(1.0)
        } else {
            None
        }
    }
}

pub struct Hdparm {}
impl DiskStatus for Hdparm {}

fn get_all_disks(sysfs: &Path) -> Vec<String> {
    let block_dir = sysfs.join("block");
    read_dir(block_dir)
        .expect("Failed to list sysfs/block")
        .map(|r| {
            let d = r.expect("Failed to get results");
            String::from(d.file_name().to_string_lossy())
        })
        .filter(|d| d.starts_with("sd"))
        .map(|d| format!("/dev/{d}"))
        .collect()
}

#[cfg(test)]
mod test {
    use std::fs::{self, create_dir};

    use tempdir::TempDir;

    use super::*;

    struct FakeHdparm {}
    impl DiskStatus for FakeHdparm {
        fn get_disk_status(&self, _disk: &str) -> Option<f64> {
            Some(0.0)
        }
    }

    #[test]
    fn it_works() {
        // prepare test
        let sysfs = TempDir::new("sysfs").unwrap();
        let block = sysfs.path().join("block");
        create_dir(&block).unwrap();
        fs::write(block.join("sda"), "").unwrap();
        let textfile_collector = TempDir::new("textfile_collector").unwrap();
        let disk_status = textfile_collector.path().join("disk_status.prom");
        let disk_query = FakeHdparm {};
        let monitor = DiskMonitor::new(sysfs.path().to_path_buf(), disk_status.to_path_buf());

        // run a single cycle
        monitor.update_metrics(&disk_query);

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
