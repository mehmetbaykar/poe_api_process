use thiserror::Error;

#[derive(Error, Debug)]
pub enum PoeError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    JsonParseFailed(#[from] serde_json::Error),

    #[error("Bot error: {0}")]
    BotError(String),

    #[error("Event error: {0}")]
    EventError(String),

    #[error("Invalid event type: {0}")]
    InvalidEventType(String),

    #[error("Event parsing failed: {0}")]
    EventParseFailed(String),

    #[error("Tool call parsing failed: {0}")]
    ToolCallParseFailed(String),

    #[error("Tool result parsing failed: {0}")]
    ToolResultParseFailed(String),

    #[error("Missing required tool call ID: {0}")]
    MissingToolCallId(String),

    // File upload related errors
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("File read error: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("File upload failed: {0}")]
    FileUploadFailed(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("File too large: {0}")]
    FileTooLarge(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}
