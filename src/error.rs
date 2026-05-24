use thiserror::Error;

#[derive(Error, Debug)]
pub enum FigError {
    #[error("Invalid magic header: expected 'fig-kiwi' or 'fig-jam.', found {0:?}")]
    InvalidMagicHeader(Vec<u8>),

    #[error("File too small: expected at least {expected} bytes, found {actual}")]
    FileTooSmall { expected: usize, actual: usize },

    #[error("Incomplete chunk at offset {offset}: expected {expected} bytes, found {actual}")]
    IncompleteChunk {
        offset: usize,
        expected: usize,
        actual: usize,
    },

    #[error("Not enough chunks: expected at least {expected}, found {actual}")]
    NotEnoughChunks { expected: usize, actual: usize },

    #[error("ZIP extraction failed: {0}")]
    ZipError(String),

    #[error("Canvas file not found in ZIP archive")]
    CanvasNotFoundInZip,

    /// Recoverable in ZIP mode: when `--node` is set it is caught per-file to
    /// skip archives lacking the node; only fatal if no file contains it.
    #[error("Node {0} not found in document")]
    NodeNotFound(crate::schema::NodeId),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("ZIP library error: {0}")]
    ZipLibraryError(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, FigError>;
