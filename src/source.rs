use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use rusqlite::{OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, ExtractionRun, Result, SourceId, Workspace};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub alias: String,
    pub media_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceRegistration {
    pub source_id: SourceId,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceContentState {
    Available,
    Changed,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct SourceStatus {
    pub source_id: SourceId,
    pub content_state: SourceContentState,
    pub size_bytes: u64,
    pub media_type: String,
    pub aliases: Vec<String>,
}

pub struct SourceRegistry<'workspace> {
    workspace: &'workspace Workspace,
}

impl<'workspace> SourceRegistry<'workspace> {
    pub(crate) fn new(workspace: &'workspace Workspace) -> Self {
        Self { workspace }
    }

    pub fn register(&self, descriptor: SourceDescriptor) -> Result<SourceRegistration> {
        let SourceDescriptor { alias, media_type } = descriptor;
        let alias_path = safe_alias(&alias)?;
        let resolved = resolve_existing_alias(self.workspace.path(), alias_path)?
            .ok_or_else(|| Error::Io(std::io::Error::from(std::io::ErrorKind::NotFound)))?;
        let mut file = File::open(resolved)?;
        let mut hasher = Sha256::new();
        let mut size_bytes = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            size_bytes = size_bytes
                .checked_add(u64::try_from(read).expect("buffer length fits u64"))
                .ok_or_else(|| Error::InvalidFact("Source is too large".to_owned()))?;
        }
        let digest = hasher.finalize();
        let stored_size = i64::try_from(size_bytes)
            .map_err(|_| Error::InvalidFact("Source is too large".to_owned()))?;

        let transaction = self.workspace.connection().unchecked_transaction()?;
        let existing_alias: Option<String> = transaction
            .query_row(
                "SELECT source_id FROM source_aliases WHERE alias = ?1",
                [&alias],
                |row| row.get(0),
            )
            .optional()?;
        let existing_source: Option<String> = transaction
            .query_row(
                "SELECT id FROM sources WHERE sha256 = ?1",
                [digest.as_slice()],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(alias_source) = existing_alias {
            if existing_source.as_deref() == Some(alias_source.as_str()) {
                transaction.commit()?;
                return Ok(SourceRegistration {
                    source_id: SourceId::from_stored(alias_source),
                });
            }
            return Err(Error::SourceAliasConflict);
        }

        let source_id = if let Some(source_id) = existing_source {
            SourceId::from_stored(source_id)
        } else {
            let source_id = SourceId::from_content_digest(digest.as_slice());
            transaction.execute(
                "INSERT INTO sources (id, sha256, size_bytes, media_type) VALUES (?1, ?2, ?3, ?4)",
                params![
                    source_id.as_str(),
                    digest.as_slice(),
                    stored_size,
                    media_type
                ],
            )?;
            source_id
        };
        transaction.execute(
            "INSERT INTO source_aliases (alias, source_id) VALUES (?1, ?2)",
            params![alias, source_id.as_str()],
        )?;
        transaction.commit()?;
        Ok(SourceRegistration { source_id })
    }

    pub fn status(&self, source_id: &SourceId) -> Result<SourceStatus> {
        let (stored_digest, stored_size, media_type): (Vec<u8>, i64, String) = self
            .workspace
            .connection()
            .query_row(
                "SELECT sha256, size_bytes, media_type FROM sources WHERE id = ?1",
                [source_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(Error::SourceNotFound)?;
        let size_bytes = u64::try_from(stored_size)
            .map_err(|_| Error::InvalidWorkspace("invalid Source size".to_owned()))?;
        let mut statement = self
            .workspace
            .connection()
            .prepare("SELECT alias FROM source_aliases WHERE source_id = ?1 ORDER BY alias")?;
        let aliases = statement
            .query_map([source_id.as_str()], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        let mut content_state = SourceContentState::Unavailable;
        for alias in &aliases {
            let Some(resolved) = resolve_existing_alias(self.workspace.path(), Path::new(alias))?
            else {
                continue;
            };
            let mut file = match File::open(resolved) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            content_state = SourceContentState::Changed;
            let mut hasher = Sha256::new();
            let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            if hasher.finalize().as_slice() == stored_digest {
                content_state = SourceContentState::Available;
                break;
            }
        }
        Ok(SourceStatus {
            source_id: source_id.clone(),
            content_state,
            size_bytes,
            media_type,
            aliases,
        })
    }

    pub fn extraction_runs(&self, source_id: &SourceId) -> Result<Vec<ExtractionRun>> {
        self.status(source_id)?;
        crate::record::load_extraction_runs(self.workspace, source_id)
    }

    pub fn find_by_alias(&self, alias: &str) -> Result<Option<SourceId>> {
        safe_alias(alias)?;
        let source_id = self
            .workspace
            .connection()
            .query_row(
                "SELECT source_id FROM source_aliases WHERE alias = ?1",
                [alias],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(source_id.map(SourceId::from_stored))
    }

    pub(crate) fn read_validated(&self, source_id: &SourceId, limit: usize) -> Result<Vec<u8>> {
        let status = self.status(source_id)?;
        if status.size_bytes > u64::try_from(limit).expect("usize fits u64") {
            return Err(Error::InputTooLarge);
        }
        let stored_digest: Vec<u8> = self.workspace.connection().query_row(
            "SELECT sha256 FROM sources WHERE id = ?1",
            [source_id.as_str()],
            |row| row.get(0),
        )?;
        for alias in status.aliases {
            let Some(path) = resolve_existing_alias(self.workspace.path(), Path::new(&alias))?
            else {
                continue;
            };
            let bytes = fs::read(path)?;
            if bytes.len() <= limit && Sha256::digest(&bytes).as_slice() == stored_digest {
                return Ok(bytes);
            }
        }
        match status.content_state {
            SourceContentState::Available | SourceContentState::Changed => {
                Err(Error::SourceContentChanged)
            }
            SourceContentState::Unavailable => Err(Error::SourceContentUnavailable),
        }
    }
}

fn safe_alias(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(Error::InvalidSourceAlias);
    }
    Ok(path)
}

fn resolve_existing_alias(root: &Path, alias: &Path) -> Result<Option<PathBuf>> {
    let canonical_root = fs::canonicalize(root)?;
    let resolved = match fs::canonicalize(root.join(alias)) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !resolved.starts_with(&canonical_root) {
        return Err(Error::InvalidSourceAlias);
    }
    Ok(Some(resolved))
}
