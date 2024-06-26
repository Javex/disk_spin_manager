use anyhow::{anyhow, Context, Result};
use log::{debug, error};
use prometheus::core::{AtomicU64, GenericCounter};
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use std::fs::{self};
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

#[derive(Debug)]
pub enum MetricMessage {
    DiskStatus { disk: String, status: f64 },
    NotifyEvent(notify::Result<notify::Event>),
    SaveFile,
}

pub struct Metrics {
    registry: Registry,
    disk_status: GaugeVec,
    notify_counter: GenericCounter<AtomicU64>,
    textfile: PathBuf,
    rx: Receiver<MetricMessage>,
}

impl Metrics {
    pub fn new(textfile: PathBuf, rx: Receiver<MetricMessage>) -> Result<Self> {
        let registry = Registry::new();
        let disk_status = GaugeVec::new(
            Opts::new("disk_status", "Status of the disk (1=active, 0=standby)"),
            &["disk"],
        )?;
        registry
            .register(Box::new(disk_status.clone()))
            .context("Failed to register disk_status")?;

        let notify_counter =
            GenericCounter::new("notify_events", "Number of events  for watched directories")?;
        registry
            .register(Box::new(notify_counter.clone()))
            .context("Failed to register notify_counter")?;

        Ok(Metrics {
            registry,
            disk_status,
            notify_counter,
            textfile,
            rx,
        })
    }

    pub fn receive_metrics(&self) -> Result<()> {
        for res in self.rx.iter() {
            self.handle_metrics_message(res)?;
        }
        Ok(())
    }

    fn handle_metrics_message(&self, msg: MetricMessage) -> Result<()> {
        debug!("Received metrics message {:?}", msg);
        match msg {
            MetricMessage::DiskStatus { disk, status } => {
                self.disk_status.with_label_values(&[&disk]).set(status)
            }
            MetricMessage::NotifyEvent(Ok(_)) => self.notify_counter.inc(),
            MetricMessage::NotifyEvent(Err(err)) => {
                error!("Error from notify event: {:?}", err);
                return Err(anyhow!(err));
            }
            MetricMessage::SaveFile => self.write_textfile()?,
        }
        Ok(())
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

#[cfg(test)]
mod test {
    use std::{fs, thread, time::Duration};

    use tempfile::TempDir;

    use crate::{
        disk_status::{test::FakeHdparm, update_disk_status},
        lsblk::test::FakeLsblk,
        watch,
    };

    use super::*;

    fn init() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    #[test]
    fn test_metrics() {
        init();
        let textfile_dir = TempDir::new().unwrap();
        let textfile = textfile_dir.path().join("disk_status.prom");
        let (tx, rx) = std::sync::mpsc::channel();
        let metrics = Metrics::new(textfile.to_path_buf(), rx).unwrap();

        tx.send(MetricMessage::DiskStatus {
            disk: String::from("/dev/sda"),
            status: 1.0,
        })
        .unwrap();
        tx.send(MetricMessage::SaveFile).unwrap();

        // Close sender
        drop(tx);

        // receive single metric
        metrics.receive_metrics().unwrap();

        // compare results
        let disk_metrics = fs::read_to_string(&textfile).unwrap();
        let expected = String::from(
            "# HELP disk_status Status of the disk (1=active, 0=standby)
# TYPE disk_status gauge
disk_status{disk=\"/dev/sda\"} 1
# HELP notify_events Number of events  for watched directories
# TYPE notify_events counter
notify_events 0\n",
        );
        assert_eq!(disk_metrics, expected);
    }

    #[test]
    fn test_end_to_end() {
        // prepare test
        init();
        let (tx, rx) = std::sync::mpsc::channel();

        // set up disk spin resources
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

        // set up notify resources
        let monitored_dir = TempDir::new().unwrap();
        let event_file = monitored_dir.path().join("text.txt");
        let watches = vec![monitored_dir.path()];
        let watcher = watch::watch(&watches, tx.clone()).unwrap();

        // emit some events by changing a file
        let _ = std::fs::remove_file(&event_file);
        std::fs::write(&event_file, b"Lorem ipsum").unwrap();

        // close the transmitting side so receiver finishes
        drop(watcher);

        // set up metrics resources
        let textfile_dir = TempDir::new().unwrap();
        let textfile = textfile_dir.path().join("disk_status.prom");
        let metrics = Metrics::new(textfile.to_path_buf(), rx).unwrap();

        // run a single disk_status cycle
        update_disk_status(&disk_query, &lsblk, &tx).unwrap();

        // Send message to save the file
        tx.send(MetricMessage::SaveFile).unwrap();

        // Briefly sleep to allow inotify to catch up
        thread::sleep(Duration::from_millis(100));

        // close this transmitter, too
        drop(tx);

        // receive all metrics
        metrics.receive_metrics().unwrap();

        // compare results
        let disk_metrics = fs::read_to_string(&textfile).unwrap();
        // it's 3 events for file create, write & close from inotify
        let expected = String::from(
            "# HELP disk_status Status of the disk (1=active, 0=standby)
# TYPE disk_status gauge
disk_status{disk=\"/dev/sda\"} 0
# HELP notify_events Number of events  for watched directories
# TYPE notify_events counter
notify_events 3\n",
        );
        assert_eq!(disk_metrics, expected);
    }
}
