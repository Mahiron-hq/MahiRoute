// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Persistent user settings.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capabilities::Capabilities;
use crate::device::Direction;
use crate::selection::Selection;

/// Layout version written to disk; bumped on every incompatible change.
pub const SETTINGS_VERSION: u32 = 1;

/// Failure to read or write the settings file.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The file system refused the operation.
    #[error("settings file {path}: {source}")]
    Io {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        source: io::Error,
    },
    /// The file is not valid settings JSON.
    #[error("settings file {path} is corrupt: {source}")]
    Corrupt {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        source: serde_json::Error,
    },
    /// The file was written by a newer version of the app.
    #[error("settings file {path} has unsupported version {found}")]
    UnsupportedVersion {
        /// File involved.
        path: PathBuf,
        /// Version found in the file.
        found: u32,
    },
}

/// Everything the app remembers between launches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Layout version of the stored data.
    pub version: u32,
    /// Chosen devices and the master switch.
    pub selection: Selection,
    /// Directions whose missing device the user has already acknowledged, so the warning stays
    /// hidden until the device returns or the choice changes.
    pub dismissed_missing: BTreeSet<Direction>,
    /// Probe results per firmware fingerprint.
    pub capabilities: BTreeMap<String, Capabilities>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            selection: Selection::default(),
            dismissed_missing: BTreeSet::new(),
            capabilities: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Probe results recorded for `fingerprint`, or the unknown defaults.
    #[must_use]
    pub fn capabilities_for(&self, fingerprint: &str) -> Capabilities {
        self.capabilities
            .get(fingerprint)
            .copied()
            .unwrap_or_default()
    }

    /// Records probe results for `fingerprint`.
    pub fn set_capabilities(&mut self, fingerprint: &str, capabilities: Capabilities) {
        self.capabilities
            .insert(fingerprint.to_owned(), capabilities);
    }

    /// Reads settings from `path`; a missing file yields the defaults.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the file cannot be read, is not valid settings JSON, or comes
    /// from a newer app version.
    pub fn load(path: &Path) -> Result<Self, StoreError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(StoreError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let settings: Self =
            serde_json::from_slice(&bytes).map_err(|source| StoreError::Corrupt {
                path: path.to_owned(),
                source,
            })?;
        if settings.version > SETTINGS_VERSION {
            return Err(StoreError::UnsupportedVersion {
                path: path.to_owned(),
                found: settings.version,
            });
        }
        Ok(settings)
    }

    /// Reads settings, moving an unreadable file aside so the next save does not destroy it.
    ///
    /// Returns the settings to use and, when the stored file had to be discarded, the reason.
    #[must_use]
    pub fn load_or_quarantine(path: &Path) -> (Self, Option<StoreError>) {
        match Self::load(path) {
            Ok(settings) => (settings, None),
            Err(error @ (StoreError::Corrupt { .. } | StoreError::UnsupportedVersion { .. })) => {
                let mut aside = path.as_os_str().to_owned();
                aside.push(".corrupt");
                // Best effort: if the rename fails the next save simply overwrites the bad file.
                let _ = fs::rename(path, aside);
                (Self::default(), Some(error))
            }
            Err(error) => (Self::default(), Some(error)),
        }
    }

    /// Writes the settings atomically: a crash leaves either the old or the new file.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Io`] when the directory, temporary file or rename fails.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        let io_error = |source| StoreError::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        let json =
            serde_json::to_vec_pretty(self).map_err(|error| io_error(io::Error::other(error)))?;
        let mut file = fs::File::create(&temporary).map_err(io_error)?;
        file.write_all(&json).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Support;
    use crate::device::{DeviceKey, DeviceKind, Direction};
    use crate::selection::Choice;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        let settings = Settings::load(&dir.path().join("settings.json")).expect("load");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("nested").join("settings.json");
        let mut settings = Settings::default();
        settings.dismissed_missing.insert(Direction::Output);
        settings.selection.input = Choice::Device(DeviceKey {
            direction: Direction::Input,
            kind: DeviceKind::BuiltinMic,
            address: "bottom".to_owned(),
        });
        settings.set_capabilities(
            "xiaomi/rosemary",
            Capabilities {
                capture_preset: Support::Ignored,
                output_strategy: Support::Honored,
            },
        );
        settings.save(&path).expect("save");
        assert_eq!(Settings::load(&path).expect("load"), settings);
    }

    #[test]
    fn save_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        Settings::default().save(&path).expect("save");
        Settings::default().save(&path).expect("save again");
        let names: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("settings.json")]);
    }

    #[test]
    fn dismissed_directions_are_stored_as_a_plain_list() {
        let mut settings = Settings::default();
        settings.dismissed_missing.insert(Direction::Output);
        settings.dismissed_missing.insert(Direction::Input);
        let json = serde_json::to_value(&settings).expect("serialize");
        assert_eq!(
            json["dismissed_missing"],
            serde_json::json!(["input", "output"])
        );
    }

    #[test]
    fn corrupt_file_is_quarantined() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        fs::write(&path, b"{ not json").expect("write");
        let (settings, error) = Settings::load_or_quarantine(&path);
        assert_eq!(settings, Settings::default());
        assert!(matches!(error, Some(StoreError::Corrupt { .. })));
        assert!(!path.exists());
        assert!(dir.path().join("settings.json.corrupt").exists());
    }

    #[test]
    fn newer_version_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        fs::write(&path, br#"{"version": 99}"#).expect("write");
        assert!(matches!(
            Settings::load(&path),
            Err(StoreError::UnsupportedVersion { found: 99, .. })
        ));
    }

    #[test]
    fn unknown_capabilities_default_to_unknown() {
        let settings = Settings::default();
        assert_eq!(
            settings.capabilities_for("anything"),
            Capabilities::default()
        );
    }

    #[test]
    fn missing_and_obsolete_fields_are_tolerated() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        // `apps` belonged to an earlier layout of the file and is simply ignored.
        fs::write(&path, br#"{"version": 1, "apps": ["com.discord"]}"#).expect("write");
        let settings = Settings::load(&path).expect("load");
        assert_eq!(settings.selection, Selection::default());
        assert!(settings.dismissed_missing.is_empty());
    }
}
