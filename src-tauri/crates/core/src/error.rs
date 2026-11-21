use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("A note can't be empty.")]
    EmptyContent,

    #[error("That note no longer exists.")]
    NotFound,

    #[error("The disk is full, so the change could not be saved. Free some space and try again.")]
    DiskFull,

    #[error("The database at {path} could not be opened: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error(
        "This database was created by a newer version of Posthaste (schema {found}, this version \
         understands up to {supported}). Update Posthaste to open it; nothing was changed."
    )]
    NewerSchema { found: i64, supported: i64 },

    #[error("Upgrading the database to schema {version} failed and was rolled back: {source}")]
    Migration {
        version: i64,
        #[source]
        source: rusqlite::Error,
    },

    #[error("That file isn't a Posthaste export: {0}")]
    InvalidImport(String),

    #[error("{0}")]
    InvalidLook(String),

    #[error("{0}")]
    InvalidAsset(String),

    #[error("Built-in looks can't be changed or deleted. Make a copy to customize one.")]
    BuiltinLook,

    #[error("You already have 50 looks. Delete one to add another.")]
    TooManyLooks,

    // Attachment messages never name the file: errors are logged, and file
    // names are user data. The UI shows the name next to the message.
    #[error("This file is {size_mb} MB. Posthaste attaches files up to 15 MB.")]
    AttachmentTooLarge { size_mb: String },

    #[error("This note already has 20 files. Remove one to add another.")]
    TooManyAttachments,

    #[error("This file is empty, so there is nothing to attach.")]
    AttachmentEmpty,

    #[error("The description is too long. Notes hold up to 100,000 characters of details.")]
    DetailsTooLong,

    #[error("This note already has {max} steps. Remove some to add more.")]
    StepLimit { max: usize },

    #[error("A step is one line of at most 500 characters.")]
    StepInvalid,

    #[error("Restore this note from the Trash to change its steps.")]
    NoteTrashed,

    #[error("A project with that name already exists.")]
    ProjectExists,

    #[error("A project name is between 1 and 40 characters.")]
    ProjectInvalid,

    #[error("Database error: {0}")]
    Sqlite(#[source] rusqlite::Error),

    #[error("File error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Could not read or write JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Stable machine-readable code for the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::EmptyContent => "empty_content",
            CoreError::NotFound => "not_found",
            CoreError::DiskFull => "disk_full",
            CoreError::Open { .. } => "db_open",
            CoreError::NewerSchema { .. } => "db_newer_schema",
            CoreError::Migration { .. } => "db_migration",
            CoreError::InvalidImport(_) => "invalid_import",
            CoreError::InvalidLook(_) => "look_invalid",
            CoreError::InvalidAsset(_) => "asset_invalid",
            CoreError::BuiltinLook => "look_builtin",
            CoreError::TooManyLooks => "look_limit",
            CoreError::AttachmentTooLarge { .. } => "attachment_too_large",
            CoreError::TooManyAttachments => "too_many_attachments",
            CoreError::AttachmentEmpty => "attachment_empty",
            CoreError::DetailsTooLong => "details_too_long",
            CoreError::StepLimit { .. } => "step_limit",
            CoreError::StepInvalid => "step_invalid",
            CoreError::NoteTrashed => "note_trashed",
            CoreError::ProjectExists => "project_exists",
            CoreError::ProjectInvalid => "project_invalid",
            CoreError::Sqlite(_) => "db",
            CoreError::Io(_) => "io",
            CoreError::Json(_) => "json",
        }
    }
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(f, _) = &e {
            if f.code == rusqlite::ErrorCode::DiskFull {
                return CoreError::DiskFull;
            }
        }
        CoreError::Sqlite(e)
    }
}
