use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Textfile path where to write metrics
    #[arg(
        long,
        default_value_t = String::from("/var/lib/node_exporter/textfile_collector/disk_status.prom"),
    )]
    pub textfile: String,

    /// Interval at which to save new metrics to textfile
    #[arg(long, default_value_t = 15)]
    pub textfile_interval: u64,

    /// Path to hdparm, defaults to finding it in PATH
    #[arg(long, default_value_t = String::from("hdparm"))]
    pub hdparm: String,

    /// Enable debug mode
    #[arg(long, default_value_t = false)]
    pub debug: bool,

    /// Refresh interval in seconds, how often to run hdparm to query disk status
    #[arg(long, default_value_t = 60)]
    pub refresh_interval: u64,

    /// Which directory to monitor for events. Repeat argument for multiple directories
    #[arg(long)]
    pub watch_directories: Vec<String>,
}
