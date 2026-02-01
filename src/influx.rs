use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct InfluxClient {
    client: Arc<Client>,
    base_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct InfluxResponse {
    results: Vec<QueryResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct QueryResult {
    #[serde(default)]
    series: Vec<Series>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Series {
    name: Option<String>,
    columns: Vec<String>,
    values: Vec<Vec<Value>>,
}

impl InfluxClient {
    pub fn new(host: String, proxy: Option<String>) -> Self {
        let mut client_builder = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(10);

        if let Some(proxy_url) = proxy {
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                client_builder = client_builder.proxy(proxy);
            }
        }

        let client = client_builder.build().unwrap_or_else(|_| Client::new());

        Self {
            client: Arc::new(client),
            base_url: format!("http://{}", host),
        }
    }

    pub async fn query(
        &self,
        query: &str,
        database: Option<&str>,
    ) -> Result<Option<(Vec<String>, Vec<Vec<Value>>)>> {
        let url = format!("{}/query", self.base_url);
        let mut params = vec![("q", query.to_string())];

        if let Some(db) = database {
            params.push(("db", db.to_string()));
        }

        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP {}: {}", response.status(), response.text().await?));
        }

        let influx_response: InfluxResponse = response.json().await?;

        if let Some(result) = influx_response.results.first() {
            if let Some(err) = &result.error {
                return Err(anyhow!("InfluxDB error: {}", err));
            }

            if let Some(series) = result.series.first() {
                return Ok(Some((series.columns.clone(), series.values.clone())));
            }
        }

        Ok(None)
    }

    pub async fn show_databases(&self) -> Result<Vec<String>> {
        let result = self.query("SHOW DATABASES", None).await?;

        if let Some((_, rows)) = result {
            Ok(rows
                .into_iter()
                .filter_map(|row| {
                    row.first().and_then(|v| v.as_str().map(String::from))
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn show_measurements(&self, database: &str) -> Result<Vec<String>> {
        let result = self.query("SHOW MEASUREMENTS", Some(database)).await?;

        if let Some((_, rows)) = result {
            Ok(rows
                .into_iter()
                .filter_map(|row| {
                    row.first().and_then(|v| v.as_str().map(String::from))
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }
}