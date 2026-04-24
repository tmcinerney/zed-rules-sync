use crate::types::*;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use heed::types::{SerdeJson, Str};
use heed::{Database, Env, EnvOpenOptions};
use std::path::PathBuf;

pub struct RulesDb {
    env: Env,
    metadata: Database<SerdeJson<PromptId>, SerdeJson<PromptMetadata>>,
    bodies: Database<SerdeJson<PromptId>, Str>,
}

pub struct RuleEntry {
    pub id: PromptId,
    pub metadata: PromptMetadata,
}

impl RulesDb {
    pub fn open(path: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create db dir: {}", path.display()))?;
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(1024 * 1024 * 1024)
                .max_dbs(4)
                .open(path)
                .with_context(|| format!("failed to open LMDB at {}", path.display()))?
        };
        let mut txn = env.write_txn()?;
        let metadata = env.create_database(&mut txn, Some("metadata.v2"))?;
        let bodies = env.create_database(&mut txn, Some("bodies.v2"))?;
        txn.commit()?;
        Ok(Self {
            env,
            metadata,
            bodies,
        })
    }

    pub fn open_readonly(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            bail!(
                "database not found at {}. Has Zed been run at least once?",
                path.display()
            );
        }
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(1024 * 1024 * 1024)
                .max_dbs(4)
                .open(path)
                .with_context(|| format!("failed to open LMDB at {}", path.display()))?
        };
        let txn = env.read_txn()?;
        let metadata = env
            .open_database(&txn, Some("metadata.v2"))?
            .context("metadata.v2 not found  -- is this a Zed prompts DB?")?;
        let bodies = env
            .open_database(&txn, Some("bodies.v2"))?
            .context("bodies.v2 not found  -- is this a Zed prompts DB?")?;
        txn.commit()?;
        Ok(Self {
            env,
            metadata,
            bodies,
        })
    }

    pub fn list_rules(&self) -> Result<Vec<RuleEntry>> {
        let txn = self.env.read_txn()?;
        let mut entries = Vec::new();
        for result in self.metadata.iter(&txn)? {
            let (id, metadata) = result?;
            entries.push(RuleEntry { id, metadata });
        }
        txn.commit()?;
        Ok(entries)
    }

    pub fn upsert_rule(&self, id: PromptId, title: &str, default: bool, body: &str) -> Result<()> {
        let metadata = PromptMetadata {
            id,
            title: Some(title.to_string()),
            default,
            saved_at: Utc::now(),
        };
        let mut txn = self.env.write_txn()?;
        self.metadata.put(&mut txn, &id, &metadata)?;
        self.bodies.put(&mut txn, &id, body)?;
        txn.commit()?;
        Ok(())
    }

    pub fn delete_rule(&self, id: PromptId) -> Result<()> {
        let mut txn = self.env.write_txn()?;
        self.metadata.delete(&mut txn, &id)?;
        self.bodies.delete(&mut txn, &id)?;
        txn.commit()?;
        Ok(())
    }

    pub fn has_rule(&self, id: &PromptId) -> Result<bool> {
        let txn = self.env.read_txn()?;
        let exists = self.metadata.get(&txn, id)?.is_some();
        txn.commit()?;
        Ok(exists)
    }
}

pub fn default_db_path() -> PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").expect("HOME not set")).join(".config")
        });
    config
        .join("zed")
        .join("prompts")
        .join("prompts-library-db.0.mdb")
}

pub fn is_zed_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "Zed"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{is_managed, prompt_id_for_filename};
    use tempfile::TempDir;

    #[test]
    fn upsert_list_delete_round_trip() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("prompts");
        let db = RulesDb::open(&db_path).unwrap();

        let id = prompt_id_for_filename("code-style.md");
        db.upsert_rule(id, "Code Style", false, "body content")
            .unwrap();

        assert!(db.has_rule(&id).unwrap());
        let entries = db.list_rules().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].metadata.title.as_deref(), Some("Code Style"));
        assert!(is_managed(
            &entries[0].id,
            entries[0].metadata.title.as_deref(),
        ));

        db.delete_rule(id).unwrap();
        assert!(!db.has_rule(&id).unwrap());
        assert_eq!(db.list_rules().unwrap().len(), 0);
    }

    #[test]
    fn readonly_open_surfaces_missing_path() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(RulesDb::open_readonly(&missing).is_err());
    }
}
