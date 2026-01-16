use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::env;
use std::time::Duration;
use tracing::{info, warn};

fn get_api_url() -> String {
    env::var("ADSPOWER_API_URL").unwrap_or_else(|_| "http://127.0.0.1:50325".to_string())
}

fn get_api_key() -> Result<String> {
    // 优先读取 ADSPOWER_API_KEY，如果没有则尝试读取 ADSPOWER_TOKEN
    env::var("ADSPOWER_API_KEY")
        .or_else(|_| env::var("ADSPOWER_TOKEN"))
        .context("缺少环境变量: ADSPOWER_API_KEY 或 ADSPOWER_TOKEN")
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
                .expect("创建 reqwest 客户端失败"),
        }
    }

    /// 底层请求发送逻辑
    async fn send_request(
        &self,
        method: &str,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let mut request_builder = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            _ => anyhow::bail!("不支持的 HTTP 方法: {}", method),
        };

        match get_api_key() {
            Ok(key) => {
                request_builder = request_builder.header("api-key", &key);
                request_builder =
                    request_builder.header("Authorization", format!("Bearer {}", key));
            }
            Err(e) => {
                warn!("获取 API Key 失败: {}, 但仍发送请求。", e);
            }
        }

        if method == "POST" {
            if let Some(data) = body {
                request_builder = request_builder.json(&data);
            }
        }

        request_builder
            .send()
            .await
            .context(format!("发送请求到 {} 失败", url))
    }

    /// 统一的 API 调用封装
    async fn call_api<T, R>(&self, method: &str, endpoint: &str, body: Option<T>) -> Result<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        info!("开始 AdsPower API 调用: {} {}", method, endpoint);

        let url = format!("{}{}", get_api_url(), endpoint);
        let body_json = body.map(|b| serde_json::to_value(b).unwrap_or(json!({})));

        let response = self.send_request(method, &url, body_json).await?;
        let resp: ApiResponse<R> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower API 错误 ({}): {}", endpoint, resp.msg);
        }

        info!("AdsPower API 调用完成: {} {}", method, endpoint);

        resp.data
            .context(format!("API {} 返回成功但无数据", endpoint))
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
        info!("开始 AdsPower API 查询调用: GET {} {:?}", endpoint, query);

        let url = format!("{}{}", get_api_url(), endpoint);
        let url_with_query = reqwest::Url::parse_with_params(&url, query)?;

        let response = self
            .send_request("GET", url_with_query.as_str(), None)
            .await?;
        let resp: ApiResponse<R> = response.json().await?;

        if resp.code != 0 {
            anyhow::bail!("AdsPower API 错误 ({}): {}", endpoint, resp.msg);
        }

        info!("AdsPower API 查询调用完成: GET {}", endpoint);

        Ok(resp.data)
    }

    pub async fn check_connectivity(&self) -> Result<()> {
        // 使用 /api/v1/user/list 接口检查连接（更可靠）
        // /status 接口可能返回空响应
        info!("正在检查 AdsPower API 连接性...");

        match self
            .call_api_with_query::<ProfileListResponse>("/api/v1/user/list", &[("page_size", "1")])
            .await
        {
            Ok(_) => {
                info!("AdsPower API 已就绪");
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{:#}", e);

                // 检查是否是连接错误
                if error_msg.contains("connection")
                    || error_msg.contains("Connection")
                    || error_msg.contains("connect")
                    || error_msg.contains("timeout")
                    || error_msg.contains("refused")
                {
                    anyhow::bail!(
                        "无法连接到 AdsPower API。\n\n\
                        请确保：\n\
                        1. AdsPower 客户端已启动\n\
                        2. AdsPower 正在监听 http://127.0.0.1:50325\n\
                        3. AdsPower 的 API 功能已启用\n\n\
                        提示：请打开 AdsPower 客户端后重试。"
                    );
                }

                // 其他错误（如 API 返回错误代码）
                anyhow::bail!(
                    "AdsPower API 返回错误：{}\n\n\
                    请检查 AdsPower 客户端状态。",
                    error_msg
                );
            }
        }
    }

    pub async fn ensure_profiles_for_workers(&self, worker_count: usize) -> Result<()> {
        info!(
            "正在为 {} 个 Worker 检查 AdsPower 配置文件...",
            worker_count
        );

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
                info!("正在创建缺失的配置文件: {}", target_name);
                let user_id = self.create_profile(&target_name).await?;
                info!("已创建配置文件 {}，ID: {}", target_name, user_id);
            } else {
                info!("配置文件已存在: {}", target_name);
            }
        }
        Ok(())
    }

    pub async fn ensure_profile_for_thread(&self, thread_index: usize) -> Result<String> {
        let profile_name = format!("auto-scanner-worker-{}", thread_index);

        // 1. Try to find the profile for this thread
        if let Some(user_id) = self.find_profile_by_username(&profile_name).await? {
            info!(
                "找到线程 {} 的 AdsPower 配置文件: {}",
                thread_index, user_id
            );
            return Ok(user_id);
        }

        // 2. Create new profile if not found
        info!("正在为线程 {} 创建新的 AdsPower 配置文件", thread_index);
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
        // 随机选择操作系统：Windows 或 Mac
        // 使用时间戳的纳秒部分来决定（避免跨 await 的 Send 问题）
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();

        // 根据文档：https://localapi-doc-zh.adspower.net/docs/BgoAbq
        // ua_system_version 支持的值：
        // - Windows: Windows 7, Windows 8, Windows 10, Windows 11
        // - Mac: Mac OS X 10, Mac OS X 11, Mac OS X 12, Mac OS X 13
        let ua_system_version = if nanos.is_multiple_of(2) {
            "Windows" // Windows 操作系统（随机版本）
        } else {
            "Mac" // Mac 操作系统（随机版本）
        };

        info!(
            "正在创建配置文件 {}，UA 系统: {}",
            username, ua_system_version
        );

        // TODO: These values are currently hardcoded for Facebook.
        // In the future, we should make them configurable based on the selected strategy.
        let domain_name = "facebook.com";
        let open_urls = ["https://www.facebook.com"];

        let mut body = json!({
            "name": username,
            "group_id": "0",
            "domain_name": domain_name,
            "open_urls": open_urls,
            "fingerprint_config": {
                "random_ua": {
                    "ua_browser": ["chrome"],
                    "ua_system_version": [ua_system_version]
                }
            }
        });

        // ADSPOWER_PROXYID is mandatory
        let proxyid = env::var("ADSPOWER_PROXYID").context("需要 ADSPOWER_PROXYID 环境变量")?;

        if let Some(obj) = body.as_object_mut() {
            obj.insert(
                "user_proxy_config".to_string(),
                json!({
                    "proxy_soft": "other",
                    "proxy_type": "noproxy",
                }),
            );
            // Although we set noproxy above as a fallback structure,
            // if we are using a specific proxyid (saved proxy), we should check API docs.
            // Usually 'proxyid' at top level is enough if it refers to a saved proxy.
            // Let's stick to what we added but make it mandatory.
            obj.insert("proxyid".to_string(), json!(proxyid));
        }

        let resp: CreateProfileResponse = self
            .call_api("POST", "/api/v1/user/create", Some(body))
            .await?;

        Ok(resp.id)
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
            .context("从启动浏览器响应中获取 WebSocket URL 失败")?;

        info!("AdsPower 浏览器已启动: {}", ws_url);
        Ok(ws_url)
    }

    pub async fn stop_browser(&self, user_id: &str) -> Result<()> {
        // 停止浏览器接口可能返回 data: null，这是正常的
        match self
            .call_api_with_query::<serde_json::Value>(
                "/api/v1/browser/stop",
                &[("user_id", user_id)],
            )
            .await
        {
            Ok(_) => {
                // 无论 data 是 Some 还是 None，只要 API 调用成功（code=0）就认为停止成功
                info!("{} 的 AdsPower 浏览器已停止", user_id);
                Ok(())
            }
            Err(e) => {
                // API 调用失败才记录警告，但仍然返回 Ok 避免影响流程
                warn!("AdsPower 停止浏览器错误: {}", e);
                Ok(())
            }
        }
    }

    pub async fn delete_profile(&self, user_id: &str) -> Result<()> {
        let body = json!({
            "user_ids": [user_id]
        });

        let _: serde_json::Value = self
            .call_api("POST", "/api/v1/user/delete", Some(body))
            .await?;

        info!("已删除 AdsPower 配置文件: {}", user_id);
        Ok(())
    }
}
