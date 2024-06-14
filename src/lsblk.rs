use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Disk {
    name: String,
    #[serde(rename = "type")]
    disk_type: String,
    rota: bool,
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<Disk>,
}

pub trait LsblkDiskList {
    fn get_disk_list(&self) -> Result<String>;
}

pub struct Lsblk {}

impl LsblkDiskList for Lsblk {
    fn get_disk_list(&self) -> Result<String> {
        let output = Command::new("lsblk")
            .arg("--nodeps")
            .arg("--scsi")
            .arg("-o")
            .arg("NAME,TYPE,ROTA")
            .arg("--json")
            .output()
            .context("Failed to execute lsblk")?;
        if !output.status.success() {
            bail!("lsblk exited with error: {:?}", output);
        } else {
            Ok(String::from_utf8(output.stdout)?)
        }
    }
}

pub fn get_all_disks(lsblk: &impl LsblkDiskList) -> Result<Vec<String>> {
    let disks = lsblk.get_disk_list()?;
    let disks: LsblkOutput = serde_json::from_str(&disks)?;
    let disks = disks.blockdevices;
    let disks: Vec<String> = disks
        .into_iter()
        .filter_map(|disk| {
            if !disk.rota {
                return None;
            }
            match disk.disk_type.as_str() {
                "disk" => Some(format!("/dev/{}", disk.name)),
                _ => None,
            }
        })
        .collect();
    Ok(disks)
}

#[cfg(test)]
pub mod test {

    use super::*;

    pub struct FakeLsblk {
        pub result: String,
    }

    impl LsblkDiskList for FakeLsblk {
        fn get_disk_list(&self) -> Result<String> {
            Ok(self.result.clone())
        }
    }

    #[test]
    fn it_works() {
        let lsblk_output = r#"
{
   "blockdevices": [
      {
         "name": "sda",
         "type": "disk",
         "rota": true
      },
      {
         "name": "sr0",
         "type": "rom",
         "rota": true
      }
   ]
}
"#;
        let lsblk = FakeLsblk {
            result: lsblk_output.to_string(),
        };

        let disks = get_all_disks(&lsblk).unwrap();
        assert_eq!(disks, vec!["/dev/sda"]);
    }
}
