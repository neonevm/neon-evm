use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde_json::json;

#[derive(Debug)]
pub struct JsonRpcClient<'a> {
    url: &'a str,
    id: AtomicU64,
}

#[derive(Debug)]
pub struct Request {
    method: &'static str,
    params: serde_json::Value,
    id: u64,
}

impl Request {
    fn to_json(&self) -> serde_json::Value {
        json!({
            "method": self.method,
            "params": self.params,
            "id": self.id,
            "jsonrpc": "2.0",
        })
    }
}

impl<'a> JsonRpcClient<'a> {
    pub const fn new(url: &'a str) -> Self {
        Self {
            url,
            id: AtomicU64::new(1),
        }
    }

    pub fn request(&self, method: &'static str, params: serde_json::Value) -> Request {
        Request { method, params, id: self.id.fetch_add(1, Ordering::Relaxed) }
    }

    pub fn send_batch(&self, requests: &[Request]) -> Result<serde_json::Value> {
        let json: Vec<serde_json::Value> = requests.iter()
            .map(|request| request.to_json())
            .collect();

        let response = reqwest::blocking::Client::new()
            .post(self.url)
            .json(&json)
            .send()?;

        if response.status() == StatusCode::OK {
            return Ok(response.json()?);
        }

        Err(
            anyhow!(
                "HTTP Error: {}, Text: {}",
                response.status(),
                response.text().unwrap_or_default(),
            )
        )
    }
}
