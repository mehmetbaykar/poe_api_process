use crate::types::*;
use crate::error::PoeError;
use async_trait::async_trait;
use futures_util::StreamExt;
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

    pub async fn stream_request(&self, request: QueryRequest) -> Result<Pin<Box<dyn Stream<Item = Result<PartialResponse, PoeError>> + Send>>, PoeError> {
        let url = format!("{}{}", BASE_URL, self.bot_name);
        
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .json(&request)
            .send()
            .await?;

        let stream = response.bytes_stream().map(|result| {
            result.map_err(PoeError::from).and_then(|chunk| {
                let chunk_str = String::from_utf8_lossy(&chunk);
                for line in chunk_str.lines() {
                    if line.starts_with("data: ") {
                        let data = &line["data: ".len()..];
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                return Ok(PartialResponse {
                                    text: text.to_string(),
                                    is_suggested_reply: false,
                                    is_replace_response: false,
                                });
                            }
                        }
                    }
                }
                Err(PoeError::BotError("無效的回應格式".to_string()))
            })
        });

        Ok(Box::pin(stream))
    }

    pub async fn get_settings(&self) -> Result<SettingsResponse, PoeError> {
        let url = format!("{}{}", BASE_URL, self.bot_name);
        
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .json(&serde_json::json!({
                "version": "1.0",
                "type": "settings"
            }))
            .send()
            .await?;

        let settings = response.json::<SettingsResponse>().await?;
        Ok(settings)
    }

    pub async fn get_model_list(&self) -> Result<Vec<String>, PoeError> {
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

        let response = self.client.post(POE_GQL_URL)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .header("Content-Type", "application/json")
            .header("poegraphql", "1")
            .json(&payload)
            .send()
            .await?;

        let data: Value = response.json().await?;
        
        let mut bot_handles = Vec::new();
        if let Some(edges) = data["data"]["exploreBotsConnection"]["edges"].as_array() {
            for edge in edges {
                if let Some(handle) = edge["node"]["handle"].as_str() {
                    bot_handles.push(handle.to_string());
                }
            }
        }

        Ok(bot_handles)
    }
}

#[async_trait]
pub trait PoeBot {
    async fn get_response(&self, request: QueryRequest) -> Result<Pin<Box<dyn Stream<Item = Result<PartialResponse, PoeError>> + Send>>, PoeError>;
    async fn get_settings(&self) -> Result<SettingsResponse, PoeError>;
    async fn get_model_list(&self) -> Result<Vec<String>, PoeError>;
}

#[async_trait]
impl PoeBot for PoeClient {
    async fn get_response(&self, request: QueryRequest) -> Result<Pin<Box<dyn Stream<Item = Result<PartialResponse, PoeError>> + Send>>, PoeError> {
        self.stream_request(request).await
    }

    async fn get_settings(&self) -> Result<SettingsResponse, PoeError> {
        self.get_settings().await
    }

    async fn get_model_list(&self) -> Result<Vec<String>, PoeError> {
        self.get_model_list().await
    }
}