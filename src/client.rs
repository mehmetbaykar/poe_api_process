use crate::types::*;
use crate::error::PoeError;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use futures_util::Stream;

const BASE_URL: &str = "https://api.poe.com/bot/";
const POE_GQL_URL: &str = "https://poe.com/api/gql_POST";

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
                let chunk_str = String::from_utf8_lossy(&chunk);
                let mut lines = chunk_str.lines();
                
                while let Some(line) = lines.next() {
                    if line.starts_with("event: ") {
                        let event_type = match &line["event: ".len()..] {
                            "text" => EventType::Text,
                            "replace_response" => EventType::ReplaceResponse,
                            "done" => EventType::Done,
                            _ => continue,
                        };
                        
                        if let Some(data_line) = lines.next() {
                            if data_line.starts_with("data: ") {
                                let data = &data_line["data: ".len()..];
                                match event_type {
                                    EventType::Text | EventType::ReplaceResponse => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                                return Ok(EventResponse {
                                                    event: EventType::Text,
                                                    data: Some(PartialResponse {
                                                        text: text.to_string(),
                                                    }),
                                                });
                                            }
                                        }
                                    },
                                    EventType::Done => {
                                        return Ok(EventResponse {
                                            event: EventType::Done,
                                            data: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 如果沒有找到有效的事件，返回一個空的文字事件
                Ok(EventResponse {
                    event: EventType::Text,
                    data: Some(PartialResponse {
                        text: String::new(),
                    }),
                })
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
            "hash": "b24b2f2f6da147b3345eec1a433ed17b6e1332df97dea47622868f41078a40cc"
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
    
    let mut model_list = Vec::new();
    if let Some(edges) = data["data"]["exploreBotsConnection"]["edges"].as_array() {
        for edge in edges {
            if let (Some(handle), Some(description)) = (edge["node"]["handle"].as_str(), edge["node"]["description"].as_str()) {
                model_list.push(ModelInfo {
                    id: handle.to_string(),
                    object: "model".to_string(),
                    description: description.to_string(),
                });
            }
        }
    }

    Ok(ModelListResponse { data: model_list })
}