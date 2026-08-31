use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLockEntry {
    pub source: Option<String>,
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub skill_path: Option<String>,
    pub skill_folder_hash: Option<String>,
    pub plugin_name: Option<String>,
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub installed_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SkillLock {
    skills: HashMap<String, SkillLockEntry>,
}

impl SkillLock {
    pub fn read(path: &Path) -> Result<Self, SkillLockError> {
        let contents = match fs::read(path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(SkillLockError::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };
        let document =
            serde_json::from_slice::<SkillLockDocument>(&contents).map_err(|source| {
                SkillLockError::Decode {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
        Ok(Self {
            skills: document.skills,
        })
    }

    pub fn skill(&self, directory_name: &str) -> Option<&SkillLockEntry> {
        self.skills.get(directory_name)
    }
}

#[derive(Debug, Deserialize)]
struct SkillLockDocument {
    #[serde(default)]
    skills: HashMap<String, SkillLockEntry>,
}

#[derive(Debug, Error)]
pub enum SkillLockError {
    #[error("skill lock file could not be read at {path:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("skill lock file is invalid at {path:?}")]
    Decode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}
