use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use disk_status_exporter::{update_metrics, Hdparm};

fn main() {
    let sysfs = Path::new("/sys");
    let textfile = Path::new("/var/lib/node_exporter/textfile_collector/disk_status.prom");
    let disk_query = Hdparm {};
    loop {
        update_metrics(&disk_query, sysfs, textfile);
        sleep(Duration::from_secs(60));
    }
}
