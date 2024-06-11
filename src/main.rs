use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use disk_status_exporter::{DiskMonitor, Hdparm};

fn main() {
    let monitor = DiskMonitor::new(
        Path::new("/sys").to_path_buf(),
        Path::new("/var/lib/node_exporter/textfile_collector/disk_status.prom").to_path_buf(),
    );
    let disk_query = Hdparm {};
    loop {
        monitor.update_metrics(&disk_query);
        sleep(Duration::from_secs(60));
    }
}
