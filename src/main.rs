use clap::Parser;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use disk_spin_manager::{
    cli::Args,
    metrics::{DiskMonitor, Hdparm},
};
use log::debug;

fn configure_logging() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
}

fn main() -> Result<()> {
    configure_logging();

    let args = Args::parse();

    let monitor = DiskMonitor::new(
        Path::new(&args.sysfs).to_path_buf(),
        Path::new(&args.textfile).to_path_buf(),
    )?;
    debug!("Created new disk monitor");
    let disk_query = Hdparm {
        path: args.hdparm.clone(),
    };
    loop {
        debug!("Updating metrics");
        monitor.update_metrics(&disk_query)?;
        debug!("Finished metrics update, sleeping");
        sleep(Duration::from_secs(60));
    }
}
