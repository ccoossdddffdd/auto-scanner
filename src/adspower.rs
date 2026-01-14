use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tracing::{info, warn};

const ADSPOWER_API_URL: &str = "http://127.0.0.1:50325";

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
        let url = format!("{}/api/v1/user/list", ADSPOWER_API_URL);
        let response = self
            .client
            .get(&url)
            .query(&[("page_size", "2000")])
            .send()
            .await
            .context("Failed to list AdsPower profiles")?;

        let resp: ApiResponse<ProfileListResponse> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower API error: {}", resp.msg);
        }

        if let Some(data) = resp.data {
            // We assume the username is stored in the 'name' field of the profile
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
        let url = format!("{}/api/v1/user/create", ADSPOWER_API_URL);
        let body = json!({
            "name": username,
            "domain_name": "facebook.com",
            "open_urls": ["https://www.facebook.com"],
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to create AdsPower profile")?;

        let resp: ApiResponse<CreateProfileResponse> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower create profile error: {}", resp.msg);
        }

        resp.data
            .map(|d| d.id)
            .context("AdsPower API returned success but no user_id")
    }

    pub async fn update_profile_for_account(&self, user_id: &str, username: &str) -> Result<()> {
        let url = format!("{}/api/v1/user/update", ADSPOWER_API_URL);
        // Clear cookies/cache by updating with empty cookie and setting name to current user for visibility
        let body = json!({
            "user_id": user_id,
            "name": format!("auto-scanner-{}", username),
            "domain_name": "facebook.com",
            // TODO: Ideally we should clear cookies here.
            // Some API versions support clearing by passing empty cookie list or specific flag.
            // For now, we just update the name to track progress.
            // Note: Start API supports clear_cache_after_closing which might be useful.
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to update AdsPower profile")?;

        let resp: ApiResponse<serde_json::Value> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower update profile error: {}", resp.msg);
        }

        Ok(())
    }

    pub async fn start_browser(&self, user_id: &str) -> Result<String> {
        let url = format!("{}/api/v1/browser/start", ADSPOWER_API_URL);
        let response = self
            .client
            .get(&url)
            .query(&[
                ("user_id", user_id),
                ("clear_cache_after_closing", "1"), // Ensure cache is cleared when stopped
                ("launch_args", "[\"--incognito\"]"), // Optional: try to force incognito if supported
            ])
            .send()
            .await
            .context("Failed to start AdsPower browser")?;

        let resp: ApiResponse<StartBrowserResponse> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower start browser error: {}", resp.msg);
        }

        let ws_url = resp
            .data
            .context("No data in start browser response")?
            .ws
            .puppeteer;

        info!("AdsPower browser started: {}", ws_url);
        Ok(ws_url)
    }

    pub async fn stop_browser(&self, user_id: &str) -> Result<()> {
        let url = format!("{}/api/v1/browser/stop", ADSPOWER_API_URL);
        let response = self
            .client
            .get(&url)
            .query(&[("user_id", user_id)])
            .send()
            .await
            .context("Failed to stop AdsPower browser")?;

        let resp: ApiResponse<serde_json::Value> = response.json().await?;

        if resp.code != 0 {
            warn!("AdsPower stop browser error: {}", resp.msg);
        } else {
            info!("AdsPower browser stopped for {}", user_id);
        }

        Ok(())
    }
}
