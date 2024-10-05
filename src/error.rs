use thiserror::Error;

#[derive(Error, Debug)]
pub enum PoeError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Failed to parse JSON: {0}")]
    JsonParseFailed(#[from] serde_json::Error),

    #[error("Bot error: {0}")]
    BotError(String),
}