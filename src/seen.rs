//! Persistence for already notified advertisement identifiers.
//!
//! A queue preserves insertion order while a set built by callers provides fast lookups.

use std::{
    collections::VecDeque,
    fs::{read_to_string, write},
    io::Error,
};

use {
    serde_json::{Error as JsonError, from_str, to_string_pretty},
    thiserror::Error,
    tracing::warn,
};

/// Error type for seen advertisement persistence.
#[derive(Debug, Error)]
pub enum SeenError {
    /// Stored identifiers could not be serialized.
    #[error("Stored identifiers could not be serialized: {0}")]
    Encode(#[from] JsonError),
    /// Stored identifiers could not be written.
    #[error("Stored identifiers could not be written: {0}")]
    Write(#[from] Error),
}

/// Loads seen advertisement identifiers.
///
/// # Arguments
///
/// * `path` - JSON file path with stored identifiers.
///
/// # Returns
///
/// * `VecDeque<String>` - Stored identifiers or an empty queue on any failure.
pub fn load(path: &str) -> VecDeque<String> {
    match read_to_string(path) {
        Ok(content) => match from_str(&content) {
            Ok(queue) => queue,
            Err(error) => {
                warn!(error = %error, path = %path, "Failed to parse seen ads file.");
                VecDeque::new()
            }
        },
        Err(error) => {
            warn!(error = %error, path = %path, "Failed to read seen ads file.");
            VecDeque::new()
        }
    }
}

/// Saves seen advertisement identifiers.
///
/// # Arguments
///
/// * `path` - JSON file path receiving stored identifiers.
/// * `ids` - Queue of seen identifiers in insertion order.
///
/// # Returns
///
/// * `Result<(), SeenError>` - Success or a descriptive persistence error.
///
/// # Errors
///
/// * `SeenError` - Serialization or file writing failed.
pub fn save(path: &str, ids: &VecDeque<String>) -> Result<(), SeenError> {
    let content = to_string_pretty(ids)?;
    Ok(write(path, content)?)
}

/// Prunes the oldest identifiers beyond the configured limit.
///
/// # Arguments
///
/// * `ids` - Queue of seen identifiers in insertion order.
/// * `max` - Maximum retained identifiers.
pub fn prune(ids: &mut VecDeque<String>, max: usize) {
    while ids.len() > max {
        if ids.pop_front().is_none() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use {
        anyhow::{Result, anyhow, ensure},
        tempfile::NamedTempFile,
    };

    use crate::seen::{load, prune, save};

    #[test]
    fn round_trip() -> Result<()> {
        let file = NamedTempFile::new()?;
        let path = file
            .path()
            .to_str()
            .ok_or_else(|| anyhow!("Temp path is not valid UTF-8."))?;
        let mut ids: VecDeque<String> = VecDeque::new();
        ids.push_back(String::from("42"));
        save(path, &ids)?;
        let loaded = load(path);
        ensure!(
            loaded == ids,
            "persistence round trip must preserve identifiers."
        );
        Ok(())
    }

    #[test]
    fn keeps_newest() -> Result<()> {
        let mut ids: VecDeque<String> = VecDeque::new();
        ids.push_back(String::from("1"));
        ids.push_back(String::from("2"));
        ids.push_back(String::from("3"));
        prune(&mut ids, 2);
        ensure!(ids.len() == 2, "pruning must retain two identifiers.");
        ensure!(
            ids.back().is_some_and(|last| last == "3"),
            "pruning must retain the newest identifier."
        );
        Ok(())
    }
}
