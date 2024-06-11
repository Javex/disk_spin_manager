use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Textfile path where to write metrics
    #[arg(
        long,
        default_value_t = String::from("/var/lib/node_exporter/textfile/disk_status.prom"),
    )]
    pub textfile: String,

    /// Path to sysfs, usually "/sys"
    #[arg(long, default_value_t = String::from("/sys"))]
    pub sysfs: String,

    /// Path to hdparm, defaults to finding it in PATH
    #[arg(long, default_value_t = String::from("hdparm"))]
    pub hdparm: String,
}
