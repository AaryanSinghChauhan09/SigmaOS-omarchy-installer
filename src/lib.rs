// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-omarchy-installer: omarchy installer component model.
//!
//! Real implementation: installers, events, validation, deterministic storage.
//! Replaces the placeholder stub.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installer {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub subject: String,
    pub detail: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OmarchyInstallerError {
    EmptyInstallerName,
    DuplicateInstaller(String),
    EmptyEventSubject,
}

impl fmt::Display for OmarchyInstallerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInstallerName => write!(f, "installer name must not be empty"),
            Self::DuplicateInstaller(id) => write!(f, "duplicate installer {:?}", id),
            Self::EmptyEventSubject => write!(f, "event subject must not be empty"),
        }
    }
}

impl std::error::Error for OmarchyInstallerError {}

pub struct OmarchyInstaller {
    pub name: String,
    installers: BTreeMap<String, Installer>,
    events: BTreeMap<String, Event>,
}

impl OmarchyInstaller {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            installers: BTreeMap::new(),
            events: BTreeMap::new(),
        }
    }

    pub fn add_installer(&mut self, item: Installer) -> Result<(), OmarchyInstallerError> {
        if item.name.trim().is_empty() {
            return Err(OmarchyInstallerError::EmptyInstallerName);
        }
        let id = item.id.clone();
        if self.installers.contains_key(&id) {
            return Err(OmarchyInstallerError::DuplicateInstaller(id.clone()));
        }
        self.installers.insert(id, item);
        Ok(())
    }

    pub fn installer_count(&self) -> usize {
        self.installers.len()
    }

    pub fn get_installer(&self, id: &str) -> Option<&Installer> {
        self.installers.get(id)
    }

    pub fn add_event(&mut self, event: Event) -> Result<(), OmarchyInstallerError> {
        if event.subject.trim().is_empty() {
            return Err(OmarchyInstallerError::EmptyEventSubject);
        }
        self.events.insert(event.id.clone(), event);
        Ok(())
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn get_event(&self, id: &str) -> Option<&Event> {
        self.events.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_installer(id: &str, name: &str) -> Installer {
        Installer {
            id: id.into(),
            name: name.into(),
            kind: "default".into(),
            detail: String::new(),
        }
    }

    #[test]
    fn test_create() {
        let m = OmarchyInstaller::new("OmarchyInstaller");
        assert_eq!(m.name, "OmarchyInstaller");
        assert_eq!(m.installer_count(), 0);
        assert_eq!(m.event_count(), 0);
    }

    #[test]
    fn test_add_installer() {
        let mut m = OmarchyInstaller::new("OmarchyInstaller");
        m.add_installer(sample_installer("a1", "Alpha")).unwrap();
        assert_eq!(m.installer_count(), 1);
        assert!(m.get_installer("a1").is_some());
    }

    #[test]
    fn test_empty_name_rejected() {
        let mut m = OmarchyInstaller::new("OmarchyInstaller");
        assert!(m.add_installer(sample_installer("x", "   ")).is_err());
    }

    #[test]
    fn test_duplicate_rejected() {
        let mut m = OmarchyInstaller::new("OmarchyInstaller");
        m.add_installer(sample_installer("x", "One")).unwrap();
        assert!(m.add_installer(sample_installer("x", "Two")).is_err());
    }

    #[test]
    fn test_event_validation() {
        let mut m = OmarchyInstaller::new("OmarchyInstaller");
        let bad = Event { id: "e1".into(), subject: "".into(), detail: String::new(), timestamp: 0 };
        assert!(m.add_event(bad).is_err());
        let good = Event { id: "e1".into(), subject: "boot".into(), detail: String::new(), timestamp: 1 };
        m.add_event(good).unwrap();
        assert_eq!(m.event_count(), 1);
    }
}
