use std::{path::Path, sync::mpsc::Sender};

use anyhow::Result;
use log::error;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::metrics::MetricMessage;

pub fn watch(watches: &[&Path], tx: Sender<MetricMessage>) -> Result<RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Err(err) = tx.send(MetricMessage::NotifyEvent(res)) {
            error!("Error sending message: {:?}", err);
        }
    })?;
    for watch in watches {
        watcher.watch(watch, RecursiveMode::NonRecursive)?;
    }

    Ok(watcher)
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use log::info;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn it_works() {
        // set up notify resources
        let monitored_dir = TempDir::new().unwrap();
        let event_file = monitored_dir.path().join("text.txt");
        let watches = vec![monitored_dir.path()];
        let (tx, rx) = std::sync::mpsc::channel();
        let watcher = watch(&watches, tx).unwrap();

        // emit some events by changing a file
        std::fs::write(event_file, b"Lorem ipsum").unwrap();

        // Briefly sleep to allow inotify to catch up
        thread::sleep(Duration::from_millis(100));

        // Ensure transmitting side is closed
        drop(watcher);

        let mut counter = 0;
        for res in rx {
            match res {
                MetricMessage::NotifyEvent(Ok(event)) => {
                    info!("event: {:?}", event);
                    counter += 1
                }
                MetricMessage::NotifyEvent(Err(e)) => {
                    panic!("watch error: {:?}", e);
                }
                _ => {}
            }
        }

        assert_eq!(counter, 3);
    }
}
