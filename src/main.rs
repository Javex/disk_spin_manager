use clap::Parser;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use disk_spin_manager::{
    cli::Args,
    lsblk::Lsblk,
    metrics::{DiskMonitor, Hdparm},
};
use log::debug;

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

    let monitor = DiskMonitor::new(Path::new(&args.textfile).to_path_buf())?;
    debug!("Created new disk monitor");
    let disk_query = Hdparm {
        path: args.hdparm.clone(),
    };
    let lsblk = Lsblk {};
    loop {
        debug!("Updating metrics");
        monitor.update_metrics(&disk_query, &lsblk)?;
        debug!("Finished metrics update, sleeping");
        sleep(Duration::from_secs(args.refresh_interval));
    }
}
