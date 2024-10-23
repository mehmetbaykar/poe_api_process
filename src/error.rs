use thiserror::Error;

#[derive(Error, Debug)]
pub enum PoeError {
    #[error("HTTP 請求失敗: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("JSON 解析失敗: {0}")]
    JsonParseFailed(#[from] serde_json::Error),

    #[error("Bot 錯誤: {0}")]
    BotError(String),

    #[error("事件錯誤: {0}")]
    EventError(String),

    #[error("無效的事件類型: {0}")]
    InvalidEventType(String),

    #[error("事件解析失敗: {0}")]
    EventParseFailed(String),
}