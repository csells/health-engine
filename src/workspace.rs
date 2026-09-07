use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, TransactionBehavior, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Analyses, Analysis, AnalysisOptions, Error, ExpungementManager, HealthRecord, RecordRevision,
    Renderer, Result, SourceRegistry, SubjectId, WorkspaceId,
};

const SCHEMA_VERSION: u32 = 1;
const MIGRATION: &str = include_str!("../migrations/001_initial.sql");
const CONTROLLED_REPORT_PATHS: [&str; 4] = [
    "reports/HEALTH_REPORT.html",
    "reports/PHYSICIAN_SUMMARY.md",
    "reports/current/health-summary.md",
    "reports/current/open-loops.md",
];

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct WorkspaceStatus {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub subject_id: SubjectId,
    pub record_revision: RecordRevision,
}

pub struct Workspace {
    root: PathBuf,
    connection: Connection,
    clock: Arc<dyn Clock>,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

impl Workspace {
    pub fn init(root: &Path, subject_id: SubjectId) -> Result<Self> {
        Self::init_with_clock(root, subject_id, Arc::new(SystemClock))
    }

    pub fn init_with_clock(
        root: &Path,
        subject_id: SubjectId,
        clock: Arc<dyn Clock>,
    ) -> Result<Self> {
        let subject_value = subject_id.as_str().to_owned();
        drop(subject_id);
        let engine_root = root.join(".health-engine");
        let database_path = engine_root.join("health.sqlite3");
        if engine_root.exists() {
            return Err(Error::WorkspaceExists(root.to_owned()));
        }

        fs::create_dir(&engine_root)?;
        restrict_permissions(&engine_root, 0o700)?;
        let mut connection = Connection::open(&database_path)?;
        restrict_permissions(&database_path, 0o600)?;
        configure_connection(&mut connection)?;
        connection.execute_batch(MIGRATION)?;
        let workspace_id = WorkspaceId::generate();
        connection.execute(
            "INSERT INTO workspace (singleton, workspace_id, subject_id, record_revision) \
             VALUES (1, ?1, ?2, 0)",
            params![workspace_id.as_str(), subject_value],
        )?;

        Ok(Self {
            root: root.to_owned(),
            connection,
            clock,
        })
    }

    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with_clock(root, Arc::new(SystemClock))
    }

    pub fn open_with_clock(root: &Path, clock: Arc<dyn Clock>) -> Result<Self> {
        let database_path = root.join(".health-engine/health.sqlite3");
        if !database_path.is_file() {
            return Err(Error::WorkspaceNotFound(root.to_owned()));
        }
        restrict_permissions(&root.join(".health-engine"), 0o700)?;
        restrict_permissions(&database_path, 0o600)?;
        cleanup_abandoned_staging(root)?;
        let mut connection = Connection::open(database_path)?;
        configure_connection(&mut connection)?;
        let workspace = Self {
            root: root.to_owned(),
            connection,
            clock,
        };
        workspace.status()?;
        workspace.complete_pending_expungement_cleanup()?;
        Ok(workspace)
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn sources(&self) -> SourceRegistry<'_> {
        SourceRegistry::new(self)
    }

    pub fn record(&self) -> HealthRecord<'_> {
        HealthRecord::new(self)
    }

    pub fn analyze(&self, options: AnalysisOptions) -> Result<Analysis> {
        crate::analysis::run(self, options)
    }

    pub fn analyses(&self) -> Analyses<'_> {
        Analyses::new(self)
    }

    pub fn renderer(&self) -> Renderer<'_> {
        Renderer::new(self)
    }

    pub fn expungements(&self) -> ExpungementManager<'_> {
        ExpungementManager::new(self)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(crate) fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    pub(crate) fn complete_pending_expungement_cleanup(&self) -> Result<()> {
        let pending: i64 = self.connection.query_row(
            "SELECT pending FROM expungement_cleanup_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )?;
        if pending == 0 {
            return Ok(());
        }
        if pending != 1 {
            return Err(Error::InvalidWorkspace(
                "invalid Expungement cleanup state".to_owned(),
            ));
        }
        self.connection
            .execute_batch("PRAGMA secure_delete = ON; PRAGMA wal_checkpoint(TRUNCATE);")?;
        cleanup_controlled_artifacts(&self.root)?;
        self.connection.execute_batch("VACUUM;")?;
        let changed = self.connection.execute(
            "UPDATE expungement_cleanup_state SET pending = 0 \
             WHERE singleton = 1 AND pending = 1",
            [],
        )?;
        if changed != 1 {
            return Err(Error::InvalidWorkspace(
                "invalid Expungement cleanup state".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn status(&self) -> Result<WorkspaceStatus> {
        self.connection
            .query_row(
                "SELECT workspace_id, subject_id, record_revision FROM workspace WHERE singleton = 1",
                [],
                |row| {
                    let workspace_id = WorkspaceId::from_stored(row.get(0)?);
                    let subject_value: String = row.get(1)?;
                    let subject_id = SubjectId::parse(&subject_value).map_err(|_| {
                        rusqlite::Error::InvalidColumnType(
                            1,
                            "subject_id".to_owned(),
                            rusqlite::types::Type::Text,
                        )
                    })?;
                    let stored_revision: i64 = row.get(2)?;
                    let revision = u64::try_from(stored_revision).map_err(|_| {
                        rusqlite::Error::IntegralValueOutOfRange(2, stored_revision)
                    })?;
                    Ok(WorkspaceStatus {
                        schema_version: SCHEMA_VERSION,
                        workspace_id,
                        subject_id,
                        record_revision: RecordRevision::from_stored(revision),
                    })
                },
            )
            .map_err(Error::from)
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path, mode: u32) -> std::io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

fn cleanup_abandoned_staging(root: &Path) -> std::io::Result<()> {
    let staging = root.join(".health-engine/staging");
    let Ok(metadata) = fs::symlink_metadata(&staging) else {
        return Ok(());
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(staging)
    } else {
        fs::remove_dir_all(staging)
    }
}

fn configure_connection(connection: &mut Connection) -> rusqlite::Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.set_transaction_behavior(TransactionBehavior::Immediate);
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;")
}

fn cleanup_controlled_artifacts(root: &Path) -> std::io::Result<()> {
    for relative in CONTROLLED_REPORT_PATHS {
        let path = root.join(relative);
        if path.is_file() || path.is_symlink() {
            fs::remove_file(path)?;
        }
    }
    cleanup_abandoned_staging(root)?;
    for suffix in ["-journal", "-wal", "-shm"] {
        let path = root.join(format!(".health-engine/health.sqlite3{suffix}"));
        if path.is_file() || path.is_symlink() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
