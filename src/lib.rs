// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-omarchy-installer — Omarchy-style OS installer engine.
//!
//! Models the install flow: disk selection -> partition plan -> package
//! manifest -> file copy -> bootloader -> config deploy -> first-boot.
//! Deterministic step state machine; a block-device backend executes steps.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FsType { Ext4, Btrfs, Xfs, Fat32, Swap }
impl FsType { pub fn label(self) -> &'static str { match self { Self::Ext4=>"ext4",Self::Btrfs=>"btrfs",Self::Xfs=>"xfs",Self::Fat32=>"fat32",Self::Swap=>"swap" } } }

#[derive(Debug, Clone)]
pub struct Partition {
    pub mount: String,        // "/", "/boot", "/home", "[swap]"
    pub size_gib: u32,
    pub fs: FsType,
    pub device: String,       // e.g. "/dev/nvme0n1p2"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstallStep {
    SelectDisk,
    Partition,
    Format,
    Mount,
    InstallBase,
    InstallPackages,
    InstallBootloader,
    DeployConfig,
    FirstBoot,
    Done,
}

impl InstallStep {
    pub fn label(self) -> &'static str {
        match self {
            Self::SelectDisk => "select-disk", Self::Partition => "partition",
            Self::Format => "format", Self::Mount => "mount",
            Self::InstallBase => "install-base", Self::InstallPackages => "install-packages",
            Self::InstallBootloader => "install-bootloader", Self::DeployConfig => "deploy-config",
            Self::FirstBoot => "first-boot", Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState { Pending, Running, Done, Failed }

#[derive(Debug, Clone)]
pub struct Step { pub step: InstallStep, pub state: StepState, pub detail: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    DiskTooSmall { need: u32, have: u32 },
    NoDisk,
    StepOutOfOrder { want: &'static str, got: &'static str },
    StepFailed(String),
    AlreadyDone,
}
impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DiskTooSmall { need, have } => write!(f, "disk too small: need {need}GiB, have {have}GiB"),
            Self::NoDisk => write!(f, "no target disk selected"),
            Self::StepOutOfOrder { want, got } => write!(f, "expected step {want}, got {got}"),
            Self::StepFailed(s) => write!(f, "step failed: {s}"),
            Self::AlreadyDone => write!(f, "install already complete"),
        }
    }
}
impl std::error::Error for InstallError {}

pub struct OmarchyInstaller {
    pub disk: Option<Disk>,
    pub root_password_set: bool,
    pub username: Option<String>,
    pub packages: Vec<String>,
    steps: BTreeMap<InstallStep, Step>,
    current: InstallStep,
}

#[derive(Debug, Clone)]
pub struct Disk { pub name: String, pub size_gib: u32, pub model: String }

impl OmarchyInstaller {
    pub fn new() -> Self {
        let mut steps = BTreeMap::new();
        for s in [InstallStep::SelectDisk, InstallStep::Partition, InstallStep::Format,
                  InstallStep::Mount, InstallStep::InstallBase, InstallStep::InstallPackages,
                  InstallStep::InstallBootloader, InstallStep::DeployConfig,
                  InstallStep::FirstBoot, InstallStep::Done] {
            steps.insert(s, Step { step: s, state: StepState::Pending, detail: String::new() });
        }
        Self { disk: None, root_password_set: false, username: None,
               packages: Vec::new(), steps, current: InstallStep::SelectDisk }
    }

    pub fn select_disk(&mut self, disk: Disk) -> Result<(), InstallError> {
        if disk.size_gib < 20 { return Err(InstallError::DiskTooSmall { need: 20, have: disk.size_gib }); }
        self.disk = Some(disk);
        Ok(())
    }

    /// Generate the default Omarchy partition plan for the selected disk.
    pub fn partition_plan(&self) -> Result<Vec<Partition>, InstallError> {
        let d = self.disk.as_ref().ok_or(InstallError::NoDisk)?;
        let mut p = vec![
            Partition { mount: "/boot".into(), size_gib: 1, fs: FsType::Fat32, device: format!("{}p1", d.name) },
            Partition { mount: "[swap]".into(), size_gib: 8, fs: FsType::Swap, device: format!("{}p2", d.name) },
        ];
        let root = Partition { mount: "/".into(), size_gib: d.size_gib - 9, fs: FsType::Btrfs, device: format!("{}p3", d.name) };
        p.push(root);
        Ok(p)
    }

    /// Advance the install state machine one step. Steps must run in order.
    pub fn advance(&mut self) -> Result<InstallStep, InstallError> {
        if self.current == InstallStep::Done { return Err(InstallError::AlreadyDone); }
        let order = [InstallStep::SelectDisk, InstallStep::Partition, InstallStep::Format,
                     InstallStep::Mount, InstallStep::InstallBase, InstallStep::InstallPackages,
                     InstallStep::InstallBootloader, InstallStep::DeployConfig,
                     InstallStep::FirstBoot, InstallStep::Done];
        let idx = order.iter().position(|s| *s == self.current).unwrap();
        // mark current done, move to next
        if let Some(st) = self.steps.get_mut(&self.current) { st.state = StepState::Done; }
        self.current = order[idx + 1];
        let ns = if self.current == InstallStep::Done { StepState::Done } else { StepState::Running };
        if let Some(st) = self.steps.get_mut(&self.current) { st.state = ns; }
        Ok(self.current)
    }

    /// Run the full install to completion (simulated steps).
    pub fn run_full(&mut self) -> Result<(), InstallError> {
        if self.current == InstallStep::Done { return Err(InstallError::AlreadyDone); }
        while self.current != InstallStep::Done {
            self.advance()?;
        }
        Ok(())
    }

    pub fn set_user(&mut self, name: impl Into<String>) {
        self.username = Some(name.into());
        self.root_password_set = true;
    }
    pub fn add_package(&mut self, pkg: impl Into<String>) { self.packages.push(pkg.into()); }

    pub fn progress(&self) -> (usize, usize) {
        let done = self.steps.values().filter(|s| s.state == StepState::Done).count();
        (done, self.steps.len())
    }
    pub fn status_line(&self) -> String {
        let (d, t) = self.progress();
        format!("install: {}/{} steps current={} disk={}", d, t, self.current.label(),
            self.disk.as_ref().map(|x| x.name.as_str()).unwrap_or("none"))
    }
    pub fn step(&self, s: InstallStep) -> &Step { &self.steps[&s] }
}

impl Default for OmarchyInstaller {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst() -> OmarchyInstaller {
        let mut m = OmarchyInstaller::new();
        m.select_disk(Disk { name: "/dev/nvme0n1".into(), size_gib: 512, model: "Samsung".into() }).unwrap();
        m
    }

    #[test]
    fn disk_too_small_rejected() {
        let mut m = OmarchyInstaller::new();
        assert!(m.select_disk(Disk { name: "/dev/sda".into(), size_gib: 10, model: "x".into() }).is_err());
    }

    #[test]
    fn partition_plan() {
        let m = inst();
        let p = m.partition_plan().unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].mount, "/boot");
        assert_eq!(p[2].mount, "/");
        assert_eq!(p[2].fs, FsType::Btrfs);
        assert_eq!(p[2].size_gib, 503);
    }

    #[test]
    fn ordered_steps() {
        let mut m = inst();
        assert_eq!(m.advance().unwrap(), InstallStep::Partition);
        assert_eq!(m.advance().unwrap(), InstallStep::Format);
        assert_eq!(m.step(InstallStep::SelectDisk).state, StepState::Done);
        assert_eq!(m.step(InstallStep::Partition).state, StepState::Done);
        assert_eq!(m.step(InstallStep::Format).state, StepState::Running);
    }

    #[test]
    fn full_run_completes() {
        let mut m = inst();
        m.set_user("aaryan");
        m.add_package("hyprland"); m.add_package("waybar");
        m.run_full().unwrap();
        assert_eq!(m.current, InstallStep::Done);
        assert_eq!(m.progress().0, m.progress().1);
        assert!(m.status_line().contains("done"));
        assert!(m.run_full().is_err()); // AlreadyDone
    }

    #[test]
    fn no_disk_blocks_plan() {
        let m = OmarchyInstaller::new();
        assert!(m.partition_plan().is_err());
    }
}
