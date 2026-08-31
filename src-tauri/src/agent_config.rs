use std::{collections::HashSet, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredAgentConfig {
    pub id: String,
    pub detector_id: Option<String>,
    pub display_name: String,
    pub agent_root: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentConfigDocument {
    agents: Vec<StoredAgentConfig>,
}

#[derive(Debug, Error)]
pub enum AgentConfigError {
    #[error("agent configuration filesystem operation {operation} failed for {path:?}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("agent configuration document is invalid")]
    Decode(#[source] serde_json::Error),
    #[error("agent configuration document could not be encoded")]
    Encode(#[source] serde_json::Error),
    #[error("agent configuration field {field} is invalid")]
    InvalidData { field: &'static str },
}

pub struct AgentConfigStore {
    path: PathBuf,
    agents: Vec<StoredAgentConfig>,
}

impl AgentConfigStore {
    pub fn open(path: PathBuf) -> Result<Self, AgentConfigError> {
        let agents = match fs::read(&path) {
            Ok(bytes) => {
                let document: AgentConfigDocument =
                    serde_json::from_slice(&bytes).map_err(AgentConfigError::Decode)?;
                validate_agents(&document.agents)?;
                document.agents
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(source) => {
                return Err(AgentConfigError::Io {
                    operation: "read",
                    path,
                    source,
                })
            }
        };
        Ok(Self { path, agents })
    }

    pub fn list(&self) -> &[StoredAgentConfig] {
        &self.agents
    }

    pub fn upsert(&mut self, agent: StoredAgentConfig) -> Result<(), AgentConfigError> {
        self.upsert_many(vec![agent])
    }

    pub fn upsert_many(&mut self, updates: Vec<StoredAgentConfig>) -> Result<(), AgentConfigError> {
        let mut agents = self.agents.clone();
        for agent in updates {
            if let Some(existing) = agents.iter_mut().find(|existing| existing.id == agent.id) {
                *existing = agent;
            } else {
                agents.push(agent);
            }
        }
        agents.sort_by(|left, right| left.id.cmp(&right.id));
        validate_agents(&agents)?;
        self.write(&agents)?;
        self.agents = agents;
        Ok(())
    }

    pub fn remove(&mut self, ids: &HashSet<String>) -> Result<(), AgentConfigError> {
        let agents = self
            .agents
            .iter()
            .filter(|agent| !ids.contains(&agent.id))
            .cloned()
            .collect::<Vec<_>>();
        validate_agents(&agents)?;
        self.write(&agents)?;
        self.agents = agents;
        Ok(())
    }

    fn write(&self, agents: &[StoredAgentConfig]) -> Result<(), AgentConfigError> {
        let document = AgentConfigDocument {
            agents: agents.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&document).map_err(AgentConfigError::Encode)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| AgentConfigError::Io {
                operation: "create_parent",
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&self.path, bytes).map_err(|source| AgentConfigError::Io {
            operation: "write",
            path: self.path.clone(),
            source,
        })
    }
}

fn validate_agents(agents: &[StoredAgentConfig]) -> Result<(), AgentConfigError> {
    let mut ids = HashSet::new();
    for agent in agents {
        if agent.id.trim().is_empty() || !ids.insert(agent.id.as_str()) {
            return Err(AgentConfigError::InvalidData { field: "id" });
        }
        if agent.display_name.trim().is_empty() {
            return Err(AgentConfigError::InvalidData {
                field: "displayName",
            });
        }
        if agent.agent_root.trim().is_empty() {
            return Err(AgentConfigError::InvalidData { field: "agentRoot" });
        }
    }
    Ok(())
}
