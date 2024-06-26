use anyhow::{bail, Context, Result};
use log::{debug, error};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::Duration;

use crate::{
    lsblk::{get_all_disks, Lsblk, LsblkDiskList},
    metrics::MetricMessage,
};

pub fn disk_status_loop(hdparm: &str, refresh_interval: u64, tx: Sender<MetricMessage>) {
    debug!("Created new disk monitor");
    let disk_query = Hdparm {
        path: String::from(hdparm),
    };

    let lsblk = Lsblk {};
    loop {
        debug!("Updating metrics");
        if let Err(err) = update_disk_status(&disk_query, &lsblk, &tx) {
            error!("Error updating disk status: {:?}", err);
            return;
        };
        debug!("Finished metrics update, sleeping");
        sleep(Duration::from_secs(refresh_interval));
    }
}

pub fn update_disk_status(
    disk_query: &impl DiskStatus,
    lsblk: &impl LsblkDiskList,
    tx: &Sender<MetricMessage>,
) -> Result<()> {
    let all_disks = get_all_disks(lsblk)?;
    debug!("Loaded all disks: {:?}", all_disks);
    for disk in all_disks {
        if let Some(status) = disk_query
            .get_disk_status(&disk)
            .context("failed to get disk status")?
        {
            tx.send(MetricMessage::DiskStatus { disk, status })?;
        }
    }
    Ok(())
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

#[cfg(test)]
pub mod test {
    use crate::lsblk::test::FakeLsblk;

    use super::*;

    fn init() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    pub struct FakeHdparm {}
    impl DiskStatus for FakeHdparm {
        fn get_disk_status(&self, _disk: &str) -> Result<Option<f64>> {
            Ok(Some(0.0))
        }
    }

    #[test]
    fn it_works() {
        // prepare test
        init();
        let lsblk_output = r#"
{
   "blockdevices": [
      {
         "name": "sda",
         "type": "disk",
         "rota": true
      },
      {
         "name": "sdb",
         "type": "disk",
         "rota": false
      },
      {
         "name": "sr0",
         "type": "rom",
         "rota": true
      }
   ]
}
"#;
        let lsblk = FakeLsblk {
            result: lsblk_output.to_string(),
        };
        let disk_query = FakeHdparm {};
        let (tx, rx) = std::sync::mpsc::channel();

        // run a single cycle
        update_disk_status(&disk_query, &lsblk, &tx).unwrap();

        // receive single message
        let msg = rx.recv().unwrap();

        if let MetricMessage::DiskStatus { disk, status } = msg {
            assert_eq!(disk, "/dev/sda");
            assert_eq!(status, 0.0);
        } else {
            panic!("invalid message: {:?}", msg);
        }
    }
}
