use once_cell::sync::Lazy;
use prometheus::{register_gauge_vec, Encoder, GaugeVec, TextEncoder};
use std::fs::{self, read_dir};
use std::io::BufWriter;
use std::path::Path;
use std::process::Command;

static DISK_STATUS: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "disk_status",
        "Status of the disk (1=active, 0=standby)",
        &["disk"]
    )
    .unwrap()
});

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

pub fn update_metrics(disk_query: &impl DiskStatus, sysfs: &Path, disk_status: &Path) {
    let all_disks = get_all_disks(sysfs);
    println!("all_disks: {:?}", all_disks);
    for disk in all_disks {
        if let Some(status) = disk_query.get_disk_status(&disk) {
            DISK_STATUS.with_label_values(&[&disk]).set(status);
        }
    }
    write_textfile(disk_status)
}

fn write_textfile(textfile: &Path) {
    let textfile = fs::File::create(textfile).unwrap();
    let mut textfile = BufWriter::new(textfile);
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut textfile).unwrap();
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
        let sysfs = TempDir::new("sysfs").unwrap();
        let block = sysfs.path().join("block");
        create_dir(&block).unwrap();
        fs::write(block.join("sda"), "").unwrap();
        let textfile_collector = TempDir::new("textfile_collector").unwrap();
        let disk_status = textfile_collector.path().join("disk_status.prom");
        let disk_query = FakeHdparm {};
        update_metrics(&disk_query, sysfs.path(), &disk_status);

        let disk_metrics = fs::read_to_string(&disk_status).unwrap();
        let expected = String::from(
            "# HELP disk_status Status of the disk (1=active, 0=standby)
# TYPE disk_status gauge
disk_status{disk=\"/dev/sda\"} 0\n",
        );
        assert_eq!(disk_metrics, expected);
    }
}
