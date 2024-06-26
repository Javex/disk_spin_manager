use clap::Parser;
use log::{debug, error};
use std::thread;
use std::{path::Path, time::Duration};

use anyhow::Result;
use disk_spin_manager::{
    cli::Args,
    disk_status::disk_status_loop,
    metrics::{MetricMessage, Metrics},
    watch,
};

fn configure_logging(args: &Args) {
    let level = if args.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };

    env_logger::builder().filter_level(level).init();
}

fn main() -> Result<()> {
    let args = Args::parse();

    configure_logging(&args);

    let (tx, rx) = std::sync::mpsc::channel();
    let monitor = Metrics::new(Path::new(&args.textfile).to_path_buf(), rx)?;

    let tx_disk_status = tx.clone();
    thread::spawn(move || {
        disk_status_loop(&args.hdparm, args.refresh_interval, tx_disk_status);
    });

    let tx_watch = tx.clone();

    let watches: Vec<&Path> = args
        .watch_directories
        .iter()
        .map(|s| Path::new(s.as_str()))
        .collect();
    // Ensure watcher isn't dropped until the end
    let _watcher = watch::watch(&watches, tx_watch)?;

    // Start thread to regularly save textfile
    let tx_save = tx.clone();
    thread::spawn(move || loop {
        if let Err(err) = tx_save.send(MetricMessage::SaveFile) {
            error!(
                "Error send message to save file, existing thread: {:?}",
                err
            );
            break;
        };
        debug!("Saved textfile");
        thread::sleep(Duration::from_secs(args.textfile_interval));
    });

    // Start receiving metrics
    monitor.receive_metrics()?;

    // Drop unused tx so it doesn't stay around
    drop(tx);

    Ok(())
}
