//! SigmaOS-omarchy-installer — Omarchy-style install CLI.
//!
//! Usage:
//!   omarchy-installer status
//!   omarchy-installer disks            (list detected disks)
//!   omarchy-installer plan <disk> <gib> (show partition plan)
//!   omarchy-installer run <disk> <gib> <user> [pkg...]  (full simulated install)
//!   omarchy-installer step             (advance one step)

use omarchy_installer::{Disk, OmarchyInstaller};
use std::process::exit;

fn usage() -> ! {
    eprintln!("usage: omarchy-installer <status|disks|plan|run|step> [args]");
    exit(2);
}

fn disks() -> Vec<Disk> {
    vec![
        Disk { name: "/dev/nvme0n1".into(), size_gib: 512, model: "Samsung 980 Pro".into() },
        Disk { name: "/dev/sda".into(), size_gib: 1000, model: "Crucial MX500".into() },
        Disk { name: "/dev/mmcblk0".into(), size_gib: 64, model: "SD Card".into() },
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() { usage(); }
    let mut inst = OmarchyInstaller::new();
    let cmd = args[0].as_str();
    let rest = &args[1..];

    let result: Result<(), String> = (|| -> Result<(), String> { match cmd {
        "status" => { println!("{}", inst.status_line()); Ok(()) }
        "disks" => {
            for d in disks() { println!("{} {}GiB {}", d.name, d.size_gib, d.model); }
            Ok(())
        }
        "plan" => {
            let name = rest.first().map(|s| s.as_str()).unwrap_or_else(|| usage());
            let gib: u32 = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            let disk = disks().into_iter().find(|d| d.name == name)
                .unwrap_or_else(|| Disk { name: name.into(), size_gib: gib, model: "?".into() });
            inst.select_disk(disk).map_err(|e| e.to_string())?;
            for p in inst.partition_plan().map_err(|e| e.to_string())? {
                println!("{:<8} {:>5}GiB {:<6} {}", p.mount, p.size_gib, p.fs.label(), p.device);
            }
            Ok(())
        }
        "step" => {
            // ensure a disk is selected for a meaningful demo
            if inst.disk.is_none() {
                inst.select_disk(Disk { name: "/dev/nvme0n1".into(), size_gib: 512, model: "demo".into() }).map_err(|e| e.to_string())?;
            }
            match inst.advance() {
                Ok(s) => { println!("advanced -> {}", s.label()); println!("{}", inst.status_line()); Ok(()) }
                Err(e) => Err(e.to_string()),
            }
        }
        "run" => {
            let name = rest.first().map(|s| s.as_str()).unwrap_or_else(|| usage());
            let gib: u32 = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
            let user = rest.get(2).map(|s| s.as_str()).unwrap_or("user");
            let disk = disks().into_iter().find(|d| d.name == name)
                .unwrap_or_else(|| Disk { name: name.into(), size_gib: gib, model: "?".into() });
            inst.select_disk(disk).map_err(|e| e.to_string())?;
            inst.set_user(user);
            for pkg in &rest[3.min(rest.len())..] { inst.add_package(pkg.clone()); }
            inst.run_full().map_err(|e| e.to_string())?;
            println!("install complete: {}", inst.status_line());
            println!("user={} packages={}", user, inst.packages.len());
            Ok(())
        }
        _ => usage(),
    } })();

    if let Err(e) = result {
        eprintln!("omarchy-installer: {e}");
        exit(1);
    }
}
