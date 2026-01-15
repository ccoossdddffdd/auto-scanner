use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

fn get_api_url() -> String {
    env::var("ADSPOWER_API_URL").unwrap_or_else(|_| "http://127.0.0.1:50325".to_string())
}

fn get_api_key() -> Option<String> {
    let key = env::var("ADSPOWER_API_KEY").ok().filter(|s| !s.is_empty());
    if key.is_some() {
        info!("Using AdsPower API Key: ***");
    } else {
        warn!("No AdsPower API Key found in environment variables");
    }
    key
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    code: i32,
    msg: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct ProfileListResponse {
    list: Vec<Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    user_id: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateProfileResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct StartBrowserResponse {
    ws: WebSocketInfo,
}

#[derive(Debug, Deserialize)]
struct WebSocketInfo {
    puppeteer: String,
}

pub struct AdsPowerClient {
    client: Client,
}

impl Default for AdsPowerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AdsPowerClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .no_proxy()
                .build()
                .expect("Failed to create reqwest client"),
        }
    }

    /// 统一的 API 调用封装
    async fn call_api<T, R>(&self, method: &str, endpoint: &str, body: Option<T>) -> Result<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        // Add 1s delay to avoid rate limiting
        sleep(Duration::from_secs(1)).await;

        let url = format!("{}{}", get_api_url(), endpoint);

        let mut request_builder = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            _ => anyhow::bail!("Unsupported HTTP method: {}", method),
        };

        // 尝试从 get_api_key 获取 key，但无论是否配置都强制设置 api-key 头
        // 如果未配置，则设置为空字符串或默认值，具体取决于 API 行为
        // 根据用户反馈，必须设置 api-key 头，即使可能为空
        if let Some(key) = get_api_key() {
            request_builder = request_builder.header("api-key", key);
        } else {
            // 如果环境变量未设置，但后端强制要求 api-key，尝试设置为空字符串
            // 或者检查是否之前读取逻辑有问题
            warn!("ADSPOWER_API_KEY is not set, but sending request anyway.");
        }

        if method == "POST" {
            if let Some(data) = body {
                request_builder = request_builder.json(&data);
            }
        }

        let response = request_builder
            .send()
            .await
            .context(format!("Failed to call AdsPower API: {}", endpoint))?;

        let resp: ApiResponse<R> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower API error ({}): {}", endpoint, resp.msg);
        }

        resp.data
            .context(format!("API {} returned success but no data", endpoint))
    }

    /// 发送 GET 请求（带查询参数）
    async fn call_api_with_query<R>(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<Option<R>>
    where
        R: serde::de::DeserializeOwned,
    {
        // Add 1s delay to avoid rate limiting
        sleep(Duration::from_secs(1)).await;

        let url = format!("{}{}", get_api_url(), endpoint);

        let mut request_builder = self.client.get(&url).query(query);

        if let Some(key) = get_api_key() {
            request_builder = request_builder.header("api-key", key);
        } else {
            warn!("ADSPOWER_API_KEY is not set, but sending request anyway.");
        }

        let response = request_builder
            .send()
            .await
            .context(format!("Failed to call AdsPower API: {}", endpoint))?;

        let resp: ApiResponse<R> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower API error ({}): {}", endpoint, resp.msg);
        }

        Ok(resp.data)
    }

    pub async fn check_connectivity(&self) -> Result<()> {
        self.call_api_with_query::<serde_json::Value>("/api/v1/user/list", &[("page_size", "1")])
            .await
            .map(|_| ())
            .context("Failed to connect to AdsPower API")
    }

    pub async fn ensure_profiles_for_workers(&self, worker_count: usize) -> Result<()> {
        info!("Checking AdsPower profiles for {} workers...", worker_count);

        let data: Option<ProfileListResponse> = self
            .call_api_with_query("/api/v1/user/list", &[("page_size", "2000")])
            .await?;

        let existing_names: HashSet<String> = if let Some(data) = data {
            data.list.into_iter().filter_map(|p| p.name).collect()
        } else {
            HashSet::new()
        };

        for i in 0..worker_count {
            let target_name = format!("auto-scanner-worker-{}", i);
            if !existing_names.contains(&target_name) {
                info!("Creating missing profile: {}", target_name);
                let user_id = self.create_profile(&target_name).await?;
                info!("Created profile {} with ID: {}", target_name, user_id);
            } else {
                info!("Profile exists: {}", target_name);
            }
        }
        Ok(())
    }

    pub async fn ensure_profile_for_thread(&self, thread_index: usize) -> Result<String> {
        let profile_name = format!("auto-scanner-worker-{}", thread_index);

        // 1. Try to find the profile for this thread
        if let Some(user_id) = self.find_profile_by_username(&profile_name).await? {
            info!(
                "Found AdsPower profile for thread {}: {}",
                thread_index, user_id
            );
            return Ok(user_id);
        }

        // 2. Create new profile if not found
        info!("Creating new AdsPower profile for thread {}", thread_index);
        self.create_profile(&profile_name).await
    }

    pub async fn ensure_single_profile(&self) -> Result<String> {
        self.ensure_profile_for_thread(0).await
    }

    async fn find_profile_by_username(&self, username: &str) -> Result<Option<String>> {
        let data: Option<ProfileListResponse> = self
            .call_api_with_query("/api/v1/user/list", &[("page_size", "2000")])
            .await?;

        if let Some(data) = data {
            for profile in data.list {
                if let Some(name) = profile.name {
                    if name == username {
                        return Ok(Some(profile.user_id));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn create_profile(&self, username: &str) -> Result<String> {
        let body = json!({
            "name": username,
            "domain_name": "facebook.com",
            "open_urls": ["https://www.facebook.com"],
        });

        let resp: CreateProfileResponse = self
            .call_api("POST", "/api/v1/user/create", Some(body))
            .await?;

        Ok(resp.id)
    }

    pub async fn update_profile_for_account(&self, user_id: &str, username: &str) -> Result<()> {
        let body = json!({
            "user_id": user_id,
            "name": format!("auto-scanner-{}", username),
            "domain_name": "facebook.com",
        });

        let _: serde_json::Value = self
            .call_api("POST", "/api/v1/user/update", Some(body))
            .await?;

        Ok(())
    }

    pub async fn start_browser(&self, user_id: &str) -> Result<String> {
        let data: Option<StartBrowserResponse> = self
            .call_api_with_query(
                "/api/v1/browser/start",
                &[
                    ("user_id", user_id),
                    ("clear_cache_after_closing", "1"),
                    ("launch_args", "[\"--incognito\"]"),
                ],
            )
            .await?;

        let ws_url = data
            .map(|d| d.ws.puppeteer)
            .context("Failed to get WebSocket URL from start browser response")?;

        info!("AdsPower browser started: {}", ws_url);
        Ok(ws_url)
    }

    pub async fn stop_browser(&self, user_id: &str) -> Result<()> {
        let result: Result<serde_json::Value> = self
            .call_api_with_query("/api/v1/browser/stop", &[("user_id", user_id)])
            .await
            .and_then(|data| data.context("No data in stop browser response"));

        match result {
            Ok(_) => {
                info!("AdsPower browser stopped for {}", user_id);
                Ok(())
            }
            Err(e) => {
                warn!("AdsPower stop browser error: {}", e);
                Ok(())
            }
        }
    }
}
