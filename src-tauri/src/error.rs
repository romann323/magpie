use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PicOrgError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("connection pool error: {0}")]
    Pool(String),

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

impl From<image::ImageError> for PicOrgError {
    fn from(e: image::ImageError) -> Self {
        PicOrgError::ImageDecode(e.to_string())
    }
}

impl From<anyhow::Error> for PicOrgError {
    fn from(e: anyhow::Error) -> Self {
        PicOrgError::Internal(e.to_string())
    }
}

impl Serialize for PicOrgError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Payload<'a> {
            code: &'a str,
            message: String,
        }
        let code = match self {
            PicOrgError::Io(_) => "io",
            PicOrgError::Db(_) => "db",
            PicOrgError::Pool(_) => "pool",
            PicOrgError::PathNotFound(_) => "path_not_found",
            PicOrgError::NotADirectory(_) => "not_a_directory",
            PicOrgError::ImageNotFound(_) => "image_not_found",
            PicOrgError::FolderNotFound(_) => "folder_not_found",
            PicOrgError::UnsupportedFormat(_) => "unsupported_format",
            PicOrgError::ImageDecode(_) => "image_decode",
            PicOrgError::MetadataRead(_) => "metadata_read",
            PicOrgError::MetadataWrite(_) => "metadata_write",
            PicOrgError::BadInput(_) => "bad_input",
            PicOrgError::Internal(_) => "internal",
        };
        Payload { code, message: self.to_string() }.serialize(s)
    }
}

pub type PicOrgResult<T> = Result<T, PicOrgError>;
