pub mod fingerprint;
pub mod types;

use crate::core::error::{AppError, AppResult};
use crate::infrastructure::adspower::fingerprint::FingerprintGenerator;
use crate::infrastructure::adspower::types::{
    CreateProfileRequest, FingerprintConfig, RandomUaConfig, UserProxyConfig,
};
use crate::infrastructure::browser_manager::BrowserEnvironmentManager;
use crate::infrastructure::proxy_pool::ProxyPoolManager;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct AdsPowerConfig {
    pub api_url: String,
    pub api_key: String,
    pub proxy_id: Option<String>,
}

impl AdsPowerConfig {
    pub fn from_env() -> AppResult<Self> {
        let api_url = std::env::var("ADSPOWER_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:50325".to_string());

        let api_key = std::env::var("ADSPOWER_API_KEY")
            .or_else(|_| std::env::var("ADSPOWER_TOKEN"))
            .map_err(|_| {
                AppError::Config("缺少环境变量: ADSPOWER_API_KEY 或 ADSPOWER_TOKEN".to_string())
            })?;

        let proxy_id = std::env::var("ADSPOWER_PROXYID").ok();

        Ok(Self {
            api_url,
            api_key,
            proxy_id,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ProfileConfig {
    pub group_id: String,
    pub domain_name: String,
    pub open_urls: Vec<String>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            group_id: "0".to_string(),
            domain_name: "facebook.com".to_string(),
            open_urls: vec!["https://www.facebook.com".to_string()],
        }
    }
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
    config: AdsPowerConfig,
    proxy_pool: Option<Arc<ProxyPoolManager>>,
}

impl AdsPowerClient {
    pub fn new(config: AdsPowerConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .no_proxy()
                .build()
                .expect("创建 reqwest 客户端失败"),
            config,
            proxy_pool: None,
        }
    }

    /// 配置代理池管理器
    pub fn with_proxy_pool(mut self, proxy_pool: Arc<ProxyPoolManager>) -> Self {
        info!("AdsPowerClient 已启用代理池管理");
        self.proxy_pool = Some(proxy_pool);
        self
    }

    /// 底层请求发送逻辑
    async fn send_request(
        &self,
        method: &str,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> AppResult<reqwest::Response> {
        let mut request_builder = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            _ => return Err(AppError::Network(format!("不支持的 HTTP 方法: {}", method))),
        };

        request_builder = request_builder.header("api-key", &self.config.api_key);
        request_builder =
            request_builder.header("Authorization", format!("Bearer {}", self.config.api_key));

        if method == "POST" {
            if let Some(data) = body {
                request_builder = request_builder.json(&data);
            }
        }

        request_builder
            .send()
            .await
            .map_err(|e| AppError::Network(format!("发送请求到 {} 失败: {}", url, e)))
    }

    /// 统一的 API 调用封装
    async fn call_api<T, R>(&self, method: &str, endpoint: &str, body: Option<T>) -> AppResult<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        info!("开始 AdsPower API 调用: {} {}", method, endpoint);

        let url = format!("{}{}", self.config.api_url, endpoint);
        let body_json = body.map(|b| serde_json::to_value(b).unwrap_or(json!({})));

        let response = self.send_request(method, &url, body_json).await?;
        let resp: ApiResponse<R> = response
            .json()
            .await
            .map_err(|e| AppError::Parse(format!("解析 API 响应失败: {}", e)))?;

        if resp.code != 0 {
            return Err(AppError::ExternalService(format!(
                "AdsPower API 错误 ({}): {}",
                endpoint, resp.msg
            )));
        }

        info!("AdsPower API 调用完成: {} {}", method, endpoint);

        resp.data
            .ok_or_else(|| AppError::ExternalService(format!("API {} 返回成功但无数据", endpoint)))
    }

    /// 发送 GET 请求（带查询参数）
    async fn call_api_with_query<R>(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> AppResult<Option<R>>
    where
        R: serde::de::DeserializeOwned,
    {
        info!("开始 AdsPower API 查询调用: GET {} {:?}", endpoint, query);

        let url = format!("{}{}", self.config.api_url, endpoint);
        let url_with_query = reqwest::Url::parse_with_params(&url, query)
            .map_err(|e| AppError::Parse(format!("构建 URL 失败: {}", e)))?;

        let response = self
            .send_request("GET", url_with_query.as_str(), None)
            .await?;
        let resp: ApiResponse<R> = response
            .json()
            .await
            .map_err(|e| AppError::Parse(format!("解析 API 响应失败: {}", e)))?;

        if resp.code != 0 {
            return Err(AppError::ExternalService(format!(
                "AdsPower API 错误 ({}): {}",
                endpoint, resp.msg
            )));
        }

        info!("AdsPower API 查询调用完成: GET {}", endpoint);

        Ok(resp.data)
    }

    pub async fn check_connectivity(&self) -> AppResult<()> {
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
                    return Err(AppError::Network(format!(
                        "无法连接到 AdsPower API ({})。\n\n\
                        请确保：\n\
                        1. AdsPower 客户端已启动\n\
                        2. AdsPower 正在监听 {}\n\
                        3. AdsPower 的 API 功能已启用\n\n\
                        提示：请打开 AdsPower 客户端后重试。",
                        self.config.api_url, self.config.api_url
                    )));
                }

                // 其他错误（如 API 返回错误代码）
                Err(AppError::ExternalService(format!(
                    "AdsPower API 返回错误：{}\n\n\
                    请检查 AdsPower 客户端状态。",
                    error_msg
                )))
            }
        }
    }

    pub async fn ensure_profiles_for_workers(
        &self,
        worker_count: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<()> {
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
                let user_id = self.create_profile(&target_name, config).await?;
                info!("已创建配置文件 {}，ID: {}", target_name, user_id);
            } else {
                info!("配置文件已存在: {}", target_name);
            }
        }
        Ok(())
    }

    pub async fn ensure_profile_for_thread(
        &self,
        thread_index: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<String> {
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
        self.create_profile(&profile_name, config).await
    }

    pub async fn ensure_single_profile(&self, config: Option<&ProfileConfig>) -> AppResult<String> {
        self.ensure_profile_for_thread(0, config).await
    }

    async fn find_profile_by_username(&self, username: &str) -> AppResult<Option<String>> {
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

    pub async fn create_profile(
        &self,
        username: &str,
        profile_config: Option<&ProfileConfig>,
    ) -> AppResult<String> {
        let ua_system_version = FingerprintGenerator::generate_random_system();

        info!(
            "正在创建配置文件 {}，UA 系统: {}",
            username, ua_system_version
        );

        let default_config = ProfileConfig::default();
        let config = profile_config.unwrap_or(&default_config);

        // 代理配置优先级：
        // 1. 代理池动态分配（如果启用）
        // 2. 环境变量 ADSPOWER_PROXYID（回退方案）
        let (user_proxy_config, proxyid) = if let Some(pool) = &self.proxy_pool {
            match pool.get_next().await {
                Some(proxy_config) => {
                    info!(
                        "从代理池分配代理: {}:{}",
                        proxy_config
                            .proxy_host
                            .as_ref()
                            .unwrap_or(&"unknown".to_string()),
                        proxy_config
                            .proxy_port
                            .as_ref()
                            .unwrap_or(&"unknown".to_string())
                    );
                    (proxy_config, None)
                }
                None => {
                    warn!("代理池无可用代理，回退到环境变量 ADSPOWER_PROXYID");
                    let proxyid = self.config.proxy_id.clone().ok_or_else(|| {
                        AppError::Config("代理池为空且缺少 ADSPOWER_PROXYID 配置".to_string())
                    })?;
                    (UserProxyConfig::with_proxyid(), Some(proxyid))
                }
            }
        } else if let Some(proxy_id) = &self.config.proxy_id {
            info!("使用环境变量配置的代理 ID: {}", proxy_id);
            (UserProxyConfig::with_proxyid(), Some(proxy_id.clone()))
        } else {
            return Err(AppError::Config(
                "未配置代理池且缺少 ADSPOWER_PROXYID 环境变量".to_string(),
            ));
        };

        let request = CreateProfileRequest {
            name: username.to_string(),
            group_id: config.group_id.clone(),
            domain_name: config.domain_name.clone(),
            open_urls: config.open_urls.clone(),
            fingerprint_config: FingerprintConfig {
                random_ua: RandomUaConfig {
                    ua_browser: vec!["chrome".to_string()],
                    ua_system_version: vec![ua_system_version.to_string()],
                },
            },
            user_proxy_config,
            proxyid,
        };

        let resp: CreateProfileResponse = self
            .call_api("POST", "/api/v1/user/create", Some(request))
            .await?;

        Ok(resp.id)
    }

    /// 为指定 Worker 创建配置文件（使用粘性代理分配）
    pub async fn create_profile_for_worker(
        &self,
        username: &str,
        worker_index: usize,
        profile_config: Option<&ProfileConfig>,
    ) -> AppResult<String> {
        let ua_system_version = FingerprintGenerator::generate_random_system();

        info!(
            "正在为 Worker {} 创建配置文件 {}，UA 系统: {}",
            worker_index, username, ua_system_version
        );

        let default_config = ProfileConfig::default();
        let config = profile_config.unwrap_or(&default_config);

        // 为 Worker 分配固定代理（粘性分配）
        let (user_proxy_config, proxyid) = if let Some(pool) = &self.proxy_pool {
            match pool.get_for_worker(worker_index).await {
                Some(proxy_config) => {
                    info!(
                        "为 Worker {} 分配固定代理: {}:{}",
                        worker_index,
                        proxy_config
                            .proxy_host
                            .as_ref()
                            .unwrap_or(&"unknown".to_string()),
                        proxy_config
                            .proxy_port
                            .as_ref()
                            .unwrap_or(&"unknown".to_string())
                    );
                    (proxy_config, None)
                }
                None => {
                    warn!("代理池无可用代理，回退到环境变量 ADSPOWER_PROXYID");
                    let proxyid = self.config.proxy_id.clone().ok_or_else(|| {
                        AppError::Config("代理池为空且缺少 ADSPOWER_PROXYID 配置".to_string())
                    })?;
                    (UserProxyConfig::with_proxyid(), Some(proxyid))
                }
            }
        } else if let Some(proxy_id) = &self.config.proxy_id {
            info!("使用环境变量配置的代理 ID: {}", proxy_id);
            (UserProxyConfig::with_proxyid(), Some(proxy_id.clone()))
        } else {
            return Err(AppError::Config(
                "未配置代理池且缺少 ADSPOWER_PROXYID 环境变量".to_string(),
            ));
        };

        let request = CreateProfileRequest {
            name: username.to_string(),
            group_id: config.group_id.clone(),
            domain_name: config.domain_name.clone(),
            open_urls: config.open_urls.clone(),
            fingerprint_config: FingerprintConfig {
                random_ua: RandomUaConfig {
                    ua_browser: vec!["chrome".to_string()],
                    ua_system_version: vec![ua_system_version.to_string()],
                },
            },
            user_proxy_config,
            proxyid,
        };

        let resp: CreateProfileResponse = self
            .call_api("POST", "/api/v1/user/create", Some(request))
            .await?;

        Ok(resp.id)
    }

    pub async fn start_browser(&self, user_id: &str) -> AppResult<String> {
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

        let ws_url = data.map(|d| d.ws.puppeteer).ok_or_else(|| {
            AppError::ExternalService("从启动浏览器响应中获取 WebSocket URL 失败".to_string())
        })?;

        info!("AdsPower 浏览器已启动: {}", ws_url);
        Ok(ws_url)
    }

    pub async fn stop_browser(&self, user_id: &str) -> AppResult<()> {
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

    pub async fn delete_profile(&self, user_id: &str) -> AppResult<()> {
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

#[async_trait]
impl BrowserEnvironmentManager for AdsPowerClient {
    async fn check_connectivity(&self) -> AppResult<()> {
        self.check_connectivity().await
    }

    async fn ensure_profiles_for_workers(
        &self,
        worker_count: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<()> {
        self.ensure_profiles_for_workers(worker_count, config).await
    }

    async fn ensure_profile_for_thread(
        &self,
        thread_index: usize,
        config: Option<&ProfileConfig>,
    ) -> AppResult<String> {
        self.ensure_profile_for_thread(thread_index, config).await
    }

    async fn start_browser(&self, user_id: &str) -> AppResult<String> {
        self.start_browser(user_id).await
    }

    async fn stop_browser(&self, user_id: &str) -> AppResult<()> {
        self.stop_browser(user_id).await
    }

    async fn delete_profile(&self, user_id: &str) -> AppResult<()> {
        self.delete_profile(user_id).await
    }
}
