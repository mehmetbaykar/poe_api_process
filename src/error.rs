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

    #[error("工具調用解析失敗: {0}")]
    ToolCallParseFailed(String),

    #[error("工具結果解析失敗: {0}")]
    ToolResultParseFailed(String),

    #[error("缺少必要的工具調用 ID: {0}")]
    MissingToolCallId(String),

    // 新增文件上傳相關錯誤
    #[error("文件不存在: {0}")]
    FileNotFound(String),

    #[error("文件讀取失敗: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("文件上傳失敗: {0}")]
    FileUploadFailed(String),

    #[error("不支持的文件類型: {0}")]
    UnsupportedFileType(String),

    #[error("文件過大: {0}")]
    FileTooLarge(String),

    #[error("無效的URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}
