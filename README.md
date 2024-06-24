Monitor whether your HDDs are spinning using Prometheus metrics. This
incredibly simple tool runs `hdparm` on a schedule and reports which disk is
spinning and which isn't. It writes those results to a text file that can be
scraped with e.g. `node-exporter`. At the moment it doesn't do more than that.

Usage:

```
disk_spin_manager --help                                                                                                                                                                                          130
Usage: disk_spin_manager [OPTIONS]

Options:
      --textfile <TEXTFILE>
          Textfile path where to write metrics [default: /var/lib/node_exporter/textfile_collector/disk_status.prom]
      --hdparm <HDPARM>
          Path to hdparm, defaults to finding it in PATH [default: hdparm]
      --debug
          Enable debug mode
      --refresh-interval <REFRESH_INTERVAL>
          Refresh interval in seconds, how often to run hdparm to query disk status [default: 60]
  -h, --help
          Print help
  -V, --version
          Print version
```

In the future, I might add features like `inotify` or `btrace` support to also
help determining what causes drives to spin up. Right now, the program is way
too basic for that. Ideally, I'd also remove the dependency on other binaries
and implement this natively, but that's also a future problem.
