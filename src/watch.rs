use std::{
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use anyhow::{anyhow, bail, Result};
use log::error;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::metrics::MetricMessage;

fn match_base_path(base_paths: &[PathBuf], paths: &[PathBuf]) -> Result<String> {
    for base in base_paths {
        for event_path in paths.iter() {
            if event_path.starts_with(base) {
                return Ok(base.to_string_lossy().to_string());
            }
        }
    }
    bail!(
        "No match for event in any of the paths. paths: {:?}, base_paths: {:?}",
        paths,
        base_paths
    )
}

fn handle_notify_event(
    watches: &[PathBuf],
    tx: &Sender<MetricMessage>,
    res: notify::Result<notify::Event>,
) {
    let message = match res {
        Ok(event) => match_base_path(watches, &event.paths),
        Err(e) => Err(anyhow!(e)),
    };
    if let Err(err) = tx.send(MetricMessage::NotifyEvent(message)) {
        error!("Error sending message: {:?}", err);
    }
}

pub fn watch(watches: Vec<&Path>, tx: Sender<MetricMessage>) -> Result<RecommendedWatcher> {
    let watches_matcher: Result<Vec<PathBuf>> = watches
        .iter()
        .map(|p| Ok(std::path::absolute(p)?))
        .collect();
    let watches_matcher = watches_matcher?;
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        handle_notify_event(&watches_matcher, &tx, res)
    })?;
    for watch in watches {
        watcher.watch(watch, RecursiveMode::Recursive)?;
    }

    Ok(watcher)
}

#[cfg(test)]
mod test {
    use std::fs;

    use log::info;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn it_works() {
        crate::metrics::test::init();
        // set up notify resources
        let monitored_dir = TempDir::new().unwrap();
        let event_file = monitored_dir.path().join("text.txt");
        let watches = vec![monitored_dir.path()];
        let (tx, rx) = std::sync::mpsc::channel();
        let watcher = watch(watches, tx).unwrap();

        // emit some events by changing a file
        std::fs::write(event_file, b"Lorem ipsum").unwrap();

        let mut counter = 0;
        // need to know exactly how many events to expect
        // making a blocking call isn't possible as it's not clear when all events have been
        // received.
        for _ in 0..3 {
            let res = rx.recv().unwrap();
            match res {
                MetricMessage::NotifyEvent(Ok(event)) => {
                    info!("event: {:?}", event);
                    assert_eq!(event, monitored_dir.path().to_string_lossy().to_string());
                    counter += 1
                }
                MetricMessage::NotifyEvent(Err(e)) => {
                    panic!("watch error: {:?}", e);
                }
                _ => {}
            }
        }

        // Ensure transmitting side is closed
        drop(watcher);

        assert_eq!(counter, 3);
    }

    #[test]
    fn test_recursive() {
        crate::metrics::test::init();
        // set up notify resources
        let monitored_dir = TempDir::new().unwrap();
        let subdir1 = monitored_dir.path().join("1");
        fs::create_dir(&subdir1).unwrap();
        let subdir2 = monitored_dir.path().join("2");
        fs::create_dir(subdir2).unwrap();
        let event_file = subdir1.join("text.txt");
        let watches = vec![subdir1.as_path()];
        let (tx, rx) = std::sync::mpsc::channel();
        let watcher = watch(watches, tx).unwrap();

        // emit some events by changing a file
        std::fs::write(event_file, b"Lorem ipsum").unwrap();

        let mut counter = 0;
        // need to know exactly how many events to expect
        // making a blocking call isn't possible as it's not clear when all events have been
        // received.
        for _ in 0..3 {
            let res = rx.recv().unwrap();
            match res {
                MetricMessage::NotifyEvent(Ok(event)) => {
                    info!("event: {:?}", event);
                    assert_eq!(event, subdir1.to_string_lossy().to_string());
                    counter += 1
                }
                MetricMessage::NotifyEvent(Err(e)) => {
                    panic!("watch error: {:?}", e);
                }
                _ => {}
            }
        }

        // Ensure transmitting side is closed
        drop(watcher);

        assert_eq!(counter, 3);
    }
}
