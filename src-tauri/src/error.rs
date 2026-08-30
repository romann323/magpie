use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(String),

    #[error("no project is open")]
    NoProjectOpen,

    #[error("path does not exist: {0}")]
    PathNotFound(String),

    #[error("path is not a directory: {0}")]
    NotADirectory(String),

    #[error("image not found: {0}")]
    ImageNotFound(i64),

    #[error("folder not found: {0}")]
    FolderNotFound(i64),

    #[error("unsupported image format: {0}")]
    UnsupportedFormat(String),

    #[error("image decode error: {0}")]
    ImageDecode(String),

    #[error("metadata read error: {0}")]
    MetadataRead(String),

    #[error("metadata write error: {0}")]
    MetadataWrite(String),

    #[error("bad input: {0}")]
    BadInput(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<image::ImageError> for AppError {
    fn from(e: image::ImageError) -> Self {
        AppError::ImageDecode(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Payload<'a> {
            code: &'a str,
            message: String,
        }
        let code = match self {
            AppError::Io(_) => "io",
            AppError::Db(_) => "db",
            AppError::Pool(_) => "pool",
            AppError::NoProjectOpen => "no_project_open",
            AppError::PathNotFound(_) => "path_not_found",
            AppError::NotADirectory(_) => "not_a_directory",
            AppError::ImageNotFound(_) => "image_not_found",
            AppError::FolderNotFound(_) => "folder_not_found",
            AppError::UnsupportedFormat(_) => "unsupported_format",
            AppError::ImageDecode(_) => "image_decode",
            AppError::MetadataRead(_) => "metadata_read",
            AppError::MetadataWrite(_) => "metadata_write",
            AppError::BadInput(_) => "bad_input",
            AppError::Internal(_) => "internal",
        };
        Payload { code, message: self.to_string() }.serialize(s)
    }
}

pub type AppResult<T> = Result<T, AppError>;
