//! EpisodicMemory — owns the CrdtDocument + synchronous atomic file
//! persistence. Single-writer-per-process, converge-on-load: opening a path
//! merges what is on disk with what we hold, so no write is ever lost.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::crdt::{CrdtDocument, Goal, GoalUpdate, Skill, TrajectoryEntry, TrajectoryType};

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode: {0}")]
    Decode(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct FilePersistence {
    path: PathBuf,
}

impl FilePersistence {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomic write: tmp file in the same directory + rename, so a crash
    /// mid-write never leaves a torn document.
    pub fn save(&self, doc: &CrdtDocument) -> Result<(), MemoryError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, doc.encode_state())?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn load(&self) -> Result<Option<CrdtDocument>, MemoryError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&self.path)?;
        Ok(Some(CrdtDocument::decode_state(&data)?))
    }
}

pub struct EpisodicMemory {
    doc: CrdtDocument,
    persistence: Option<FilePersistence>,
}

impl EpisodicMemory {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            doc: CrdtDocument::new(agent_id),
            persistence: None,
        }
    }

    /// Open (or create) a durable memory at `path`. The loaded document keeps
    /// its original agent_id — identity survives across CLI invocations.
    pub fn open(path: impl Into<PathBuf>, default_agent_id: &str) -> Result<Self, MemoryError> {
        let persistence = FilePersistence::new(path);
        let doc = match persistence.load()? {
            Some(doc) => doc,
            None => CrdtDocument::new(default_agent_id),
        };
        Ok(Self {
            doc,
            persistence: Some(persistence),
        })
    }

    /// Open an existing memory file. Fails closed when the path is missing —
    /// used by read-only MCP so a wrong cwd cannot invent an empty "healthy"
    /// document that hides the real kernel trajectory.
    pub fn open_existing(
        path: impl Into<PathBuf>,
        default_agent_id: &str,
    ) -> Result<Self, MemoryError> {
        let path = path.into();
        let persistence = FilePersistence::new(&path);
        let doc = match persistence.load()? {
            Some(doc) => doc,
            None => {
                return Err(MemoryError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "memory file missing at {} — set PERAM_MEMORY to an absolute path and run runtime load/tick first",
                        path.display()
                    ),
                )));
            }
        };
        let _ = default_agent_id; // identity comes from the on-disk document
        Ok(Self {
            doc,
            persistence: Some(persistence),
        })
    }

    pub fn doc(&self) -> &CrdtDocument {
        &self.doc
    }

    pub fn doc_mut(&mut self) -> &mut CrdtDocument {
        &mut self.doc
    }

    pub fn agent_id(&self) -> &str {
        &self.doc.agent_id
    }

    /// Merge another full state (peer sync / reconcile-before-save).
    pub fn merge_update(&mut self, update: &[u8]) -> Result<(), MemoryError> {
        let other = CrdtDocument::decode_state(update)?;
        self.doc.merge(&other);
        Ok(())
    }

    /// Reconcile with whatever is on disk (another process may have written
    /// since we opened), then persist. Returns false when no persistence.
    pub fn sync_and_save(&mut self) -> Result<bool, MemoryError> {
        let Some(p) = &self.persistence else {
            return Ok(false);
        };
        if let Some(on_disk) = p.load()? {
            if on_disk.hash != self.doc.hash {
                self.doc.merge(&on_disk);
            }
        }
        p.save(&self.doc)?;
        Ok(true)
    }

    pub fn save(&mut self) -> Result<bool, MemoryError> {
        match &self.persistence {
            Some(p) => {
                p.save(&self.doc)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub fn append(
        &mut self,
        entry_type: TrajectoryType,
        content: serde_json::Value,
        coherence: f32,
    ) -> String {
        let entry = TrajectoryEntry {
            id: String::new(),
            timestamp: chrono::Utc::now(),
            entry_type,
            content,
            coherence,
            agent_id: String::new(),
            metadata: Default::default(),
        };
        self.doc.append_trajectory(entry)
    }

    pub fn add_skill(&mut self, skill: Skill) {
        self.doc.add_skill(skill);
    }

    pub fn get_skill(&self, name: &str) -> Option<&Skill> {
        self.doc.skills.get(name)
    }

    pub fn add_goal(&mut self, goal: Goal) -> String {
        self.doc.add_goal(goal)
    }

    pub fn update_goal(&mut self, id: &str, updates: GoalUpdate) -> bool {
        self.doc.update_goal(id, updates)
    }

    pub fn set_context(&mut self, key: &str, value: serde_json::Value) {
        self.doc.set_context(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_roundtrip_keeps_identity_and_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mem.json");

        let agent = {
            let mut mem = EpisodicMemory::open(&path, "peram-swarm").unwrap();
            mem.append(
                TrajectoryType::Observation,
                serde_json::json!({"content": "hello"}),
                0.6,
            );
            mem.save().unwrap();
            mem.agent_id().to_string()
        };

        let mem = EpisodicMemory::open(&path, "peram-swarm").unwrap();
        assert_eq!(mem.agent_id(), agent, "identity survives reload");
        assert_eq!(mem.doc().trajectory.len(), 1);
    }

    #[test]
    fn sync_and_save_converges_concurrent_writers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mem.json");

        let mut a = EpisodicMemory::open(&path, "writer-a").unwrap();
        a.append(
            TrajectoryType::Observation,
            serde_json::json!({"from": "a"}),
            0.5,
        );
        a.save().unwrap();

        // Second process opens after A saved, writes its own entry.
        let mut b = EpisodicMemory::open(&path, "writer-b").unwrap();
        b.append(
            TrajectoryType::Action,
            serde_json::json!({"from": "b"}),
            0.5,
        );
        b.save().unwrap();

        // A's in-memory doc is now stale; sync_and_save must not lose B's entry.
        a.append(
            TrajectoryType::Observation,
            serde_json::json!({"from": "a2"}),
            0.5,
        );
        a.sync_and_save().unwrap();

        let disk = EpisodicMemory::open(&path, "reader").unwrap();
        assert_eq!(disk.doc().trajectory.len(), 3, "no write lost: a, b, a2");
    }
}
