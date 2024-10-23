use crate::types::*;
use crate::error::PoeError;
use bytes::BytesMut;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use futures_util::Stream;

const BASE_URL: &str = "https://api.poe.com/bot/";
const POE_GQL_URL: &str = "https://poe.com/api/gql_POST";
const POE_GQL_MODEL_HASH: &str = "b24b2f2f6da147b3345eec1a433ed17b6e1332df97dea47622868f41078a40cc";

pub struct PoeClient {
    client: Client,
    bot_name: String,
    access_key: String,
}

impl PoeClient {
    pub fn new(bot_name: &str, access_key: &str) -> Self {
        Self {
            client: Client::new(),
            bot_name: bot_name.to_string(),
            access_key: access_key.to_string(),
        }
    }

    pub async fn stream_request(&self, request: QueryRequest) -> Result<Pin<Box<dyn Stream<Item = Result<EventResponse, PoeError>> + Send>>, PoeError> {
        let url = format!("{}{}", BASE_URL, self.bot_name);
        
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .json(&request)
            .send()
            .await?;
    
        let stream = response.bytes_stream().map(move |result| {
            result.map_err(PoeError::from).and_then(|chunk| {
                let mut buffer = BytesMut::new();
                buffer.extend_from_slice(&chunk);
                
                let mut events = Vec::new();
                
                while let Some(i) = buffer.iter().position(|&b| b == b'\n') {
                    let line = buffer.split_to(i + 1);
                    let line_str = std::str::from_utf8(&line)
                        .map_err(|e| PoeError::EventParseFailed(format!("無法解析事件文字: {}", e)))?;
                    
                    if line_str.starts_with("event: ") {
                        let event_type = match &line_str["event: ".len()..].trim() {
                            &"text" => EventType::Text,
                            &"replace_response" => EventType::ReplaceResponse,
                            &"done" => EventType::Done,
                            &"error" => EventType::Error,
                            unknown => {
                                return Err(PoeError::InvalidEventType(format!(
                                    "收到未知的事件類型: {}", unknown
                                )));
                            }
                        };
                        
                        if let Some(i) = buffer.iter().position(|&b| b == b'\n') {
                            let data_line = buffer.split_to(i + 1);
                            let data_str = std::str::from_utf8(&data_line)
                                .map_err(|e| PoeError::EventParseFailed(format!("無法解析事件資料: {}", e)))?;
                            
                            if data_str.starts_with("data: ") {
                                let data = &data_str["data: ".len()..].trim();
                                match event_type {
                                    EventType::Text | EventType::ReplaceResponse => {
                                        let json = serde_json::from_str::<Value>(data)
                                            .map_err(|e| PoeError::EventError(format!("JSON 解析失敗: {}", e)))?;
                                                
                                        let text = json.get("text")
                                            .and_then(Value::as_str)
                                            .ok_or_else(|| PoeError::EventError("事件資料缺少 text 欄位".to_string()))?;

                                        events.push(EventResponse {
                                            event: event_type,
                                            data: Some(PartialResponse {
                                                text: text.to_string(),
                                            }),
                                            error: None,
                                        });
                                    },
                                    EventType::Error => {
                                        let json: Value = serde_json::from_str(data)
                                            .map_err(|e| PoeError::EventError(format!("JSON 解析失敗: {}", e)))?;
                                        
                                        let text = json.get("text")
                                            .and_then(Value::as_str)
                                            .ok_or_else(|| PoeError::EventError("錯誤事件缺少 text 欄位".to_string()))?;
                                        
                                        let allow_retry = json.get("allow_retry")
                                            .and_then(Value::as_bool)
                                            .unwrap_or(false);

                                        events.push(EventResponse {
                                            event: EventType::Error,
                                            data: None,
                                            error: Some(ErrorResponse {
                                                text: text.to_string(),
                                                allow_retry,
                                            }),
                                        });
                                    },
                                    EventType::Done => {
                                        events.push(EventResponse {
                                            event: EventType::Done,
                                            data: None,
                                            error: None,
                                        });
                                    }
                                }
                            } else {
                                return Err(PoeError::EventError("無效的事件資料格式".to_string()));
                            }
                        } else {
                            return Err(PoeError::EventError("事件資料不完整".to_string()));
                        }
                    }
                }
                
                if events.is_empty() {
                    events.push(EventResponse {
                        event: EventType::Text,
                        data: Some(PartialResponse {
                            text: String::new(),
                        }),
                        error: None,
                    });
                }
                
                Ok(events)
            })
        })
        .flat_map(|result| {
            futures_util::stream::iter(match result {
                Ok(events) => events.into_iter().map(Ok).collect::<Vec<_>>(),
                Err(e) => vec![Err(e)],
            })
        });
    
        Ok(Box::pin(stream))
    }
}

pub async fn get_model_list(language_code: Option<&str>) -> Result<ModelListResponse, PoeError> {
    let client = Client::new();
    let payload = serde_json::json!({
        "queryName": "ExploreBotsListPaginationQuery",
        "variables": {
            "categoryName": "defaultCategory",
            "count": 150
        },
        "extensions": {
            "hash": POE_GQL_MODEL_HASH
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36"));
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("poegraphql", HeaderValue::from_static("1"));

    if let Some(code) = language_code {
        headers.insert(COOKIE, HeaderValue::from_str(&format!("Poe-Language-Code={}", code)).map_err(|e| PoeError::BotError(e.to_string()))?);
    }

    let response = client.post(POE_GQL_URL)
        .headers(headers)
        .json(&payload)
        .send()
        .await?;

    let data: Value = response.json().await?;
    
    let mut model_list = Vec::with_capacity(150);
    if let Some(edges) = data["data"]["exploreBotsConnection"]["edges"].as_array() {
        for edge in edges {
            if let Some(handle) = edge["node"]["handle"].as_str() {
                model_list.push(ModelInfo {
                    id: handle.to_string(),
                    object: "model".to_string(),
                    created: 0,
                    owned_by: "poe".to_string(),
                });
            }
        }
    }

    Ok(ModelListResponse { data: model_list })
}