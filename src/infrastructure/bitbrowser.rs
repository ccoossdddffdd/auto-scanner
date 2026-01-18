pub mod fingerprint;
pub mod types;

use crate::core::error::{AppError, AppResult};
use crate::infrastructure::adspower::ProfileConfig;
use crate::infrastructure::bitbrowser::fingerprint::FingerprintGenerator;
use crate::infrastructure::bitbrowser::types::{
    BrowserFingerPrint, CreateProfileRequest, ProxyConfig,
};
use crate::infrastructure::browser_manager::BrowserEnvironmentManager;
use crate::infrastructure::proxy_pool::ProxyPoolManager;
use async_trait::async_trait;
use futures::future::join_all;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct BitBrowserConfig {
    pub api_url: String,
    pub api_key: Option<String>,
}

impl BitBrowserConfig {
    pub fn from_env() -> AppResult<Self> {
        let api_url = std::env::var("BITBROWSER_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:54345".to_string());

        let api_key = std::env::var("BITBROWSER_API_KEY").ok();

        Ok(Self { api_url, api_key })
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    msg: Option<String>,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct ProfileListResponse {
    list: Vec<Profile>,
}

#[derive(Debug, Deserialize)]
struct Profile {
    id: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateProfileResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StartBrowserResponse {
    http: String,
    ws: String,
}

#[derive(Clone)]
pub struct BitBrowserClient {
    client: Client,
    config: BitBrowserConfig,
    proxy_pool: Option<Arc<ProxyPoolManager>>,
}

impl BitBrowserClient {
    pub fn new(config: BitBrowserConfig) -> AppResult<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .no_proxy()
                .build()
                .map_err(|e| AppError::Config(format!("创建 reqwest 客户端失败: {}", e)))?,
            config,
            proxy_pool: None,
        })
    }

    /// 配置代理池管理器
    pub fn with_proxy_pool(mut self, proxy_pool: Arc<ProxyPoolManager>) -> Self {
        info!("BitBrowserClient 已启用代理池管理");
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

        // 添加 API Key 鉴权头
        if let Some(api_key) = &self.config.api_key {
            request_builder = request_builder.header("X-API-KEY", api_key);
        }

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
        info!("开始 BitBrowser API 调用: {} {}", method, endpoint);

        let url = format!("{}{}", self.config.api_url, endpoint);
        let body_json = body.map(|b| serde_json::to_value(b).unwrap_or(json!({})));

        let response = self.send_request(method, &url, body_json).await?;
        let resp: ApiResponse<R> = response
            .json()
            .await
            .map_err(|e| AppError::Parse(format!("解析 API 响应失败: {}", e)))?;

        if !resp.success {
            return Err(AppError::ExternalService(format!(
                "BitBrowser API 错误 ({}): {}",
                endpoint,
                resp.msg.unwrap_or_else(|| "未知错误".to_string())
            )));
        }

        info!("BitBrowser API 调用完成: {} {}", method, endpoint);

        resp.data
            .ok_or_else(|| AppError::ExternalService(format!("API {} 返回成功但无数据", endpoint)))
    }

    pub async fn check_connectivity(&self) -> AppResult<()> {
        info!("正在检查 BitBrowser API 连接性...");

        let request_body = json!({
            "page": 0,
            "pageSize": 1
        });

        match self
            .call_api::<serde_json::Value, ProfileListResponse>(
                "POST",
                "/browser/list",
                Some(request_body),
            )
            .await
        {
            Ok(_) => {
                info!("BitBrowser API 已就绪");
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{:#}", e);

                if error_msg.contains("connection")
                    || error_msg.contains("Connection")
                    || error_msg.contains("connect")
                    || error_msg.contains("timeout")
                    || error_msg.contains("refused")
                {
                    return Err(AppError::Network(format!(
                        "无法连接到 BitBrowser API ({})。\n\n\
                        请确保：\n\
                        1. BitBrowser 客户端已启动\n\
                        2. BitBrowser 正在监听 {}\n\
                        3. BitBrowser 的 API 功能已启用\n\n\
                        提示：请打开 BitBrowser 客户端后重试。",
                        self.config.api_url, self.config.api_url
                    )));
                }

                Err(AppError::ExternalService(format!(
                    "BitBrowser API 返回错误：{}\n\n\
                    请检查 BitBrowser 客户端状态。",
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
            "正在为 {} 个 Worker 检查 BitBrowser 配置文件...",
            worker_count
        );

        let request_body = json!({
            "page": 0,
            "pageSize": 2000
        });
        let data: ProfileListResponse = self
            .call_api("POST", "/browser/list", Some(request_body))
            .await?;

        let existing_names: HashSet<String> =
            data.list.into_iter().filter_map(|p| p.name).collect();

        let mut futures = Vec::new();

        for i in 0..worker_count {
            let target_name = format!("auto-scanner-worker-{}", i);
            if !existing_names.contains(&target_name) {
                let client = self.clone();
                let config = config.cloned();
                futures.push(async move {
                    info!("正在创建缺失的配置文件: {}", target_name);
                    match client
                        .create_profile(&target_name, config.as_ref(), i)
                        .await
                    {
                        Ok(id) => info!("已创建配置文件 {}，ID: {}", target_name, id),
                        Err(e) => warn!("创建配置文件 {} 失败: {}", target_name, e),
                    }
                });
            } else {
                info!("配置文件已存在: {}", target_name);
            }
        }

        join_all(futures).await;
        Ok(())
    }

    pub async fn ensure_profile_for_thread(
        &self,
        thread_index: usize,
        _config: Option<&ProfileConfig>,
    ) -> AppResult<String> {
        let profile_name = format!("auto-scanner-worker-{}", thread_index);

        // 1. 尝试查找该线程的配置文件
        if let Some(browser_id) = self.find_profile_by_name(&profile_name).await? {
            info!(
                "找到线程 {} 的 BitBrowser 配置文件: {}",
                thread_index, browser_id
            );
            return Ok(browser_id);
        }

        // 2. 如果未找到，创建新配置文件
        info!("正在为线程 {} 创建新的 BitBrowser 配置文件", thread_index);
        self.create_profile(&profile_name, None, thread_index).await
    }

    async fn find_profile_by_name(&self, name: &str) -> AppResult<Option<String>> {
        let request_body = json!({
            "page": 0,
            "pageSize": 2000
        });
        let data: ProfileListResponse = self
            .call_api("POST", "/browser/list", Some(request_body))
            .await?;

        for profile in data.list {
            if let Some(profile_name) = profile.name {
                if profile_name == name {
                    return Ok(Some(profile.id));
                }
            }
        }

        Ok(None)
    }

    async fn create_profile(
        &self,
        username: &str,
        profile_config: Option<&ProfileConfig>,
        worker_index: usize,
    ) -> AppResult<String> {
        let chrome_version = FingerprintGenerator::generate_random_chrome_version();

        info!(
            "正在创建配置文件 {}，Chrome 版本: {}",
            username, chrome_version
        );

        let default_config = ProfileConfig::default();
        let config = profile_config.unwrap_or(&default_config);

        // 检查是否配置了动态 IP URL
        let dynamic_ip_url = std::env::var("DYNAMIC_IP_URL").ok();

        // 为 Worker 分配代理配置
        let proxy_config = if let Some(url) = dynamic_ip_url {
            info!("为 Worker {} 使用动态 IP: {}", worker_index, url);
            ProxyConfig::with_dynamic_ip(url)
        } else if let Some(pool) = &self.proxy_pool {
            match pool.get_for_worker(worker_index).await {
                Some(adspower_proxy) => {
                    info!(
                        "为 Worker {} 分配固定代理: {}:{}",
                        worker_index,
                        adspower_proxy
                            .proxy_host
                            .as_ref()
                            .unwrap_or(&"unknown".to_string()),
                        adspower_proxy
                            .proxy_port
                            .as_ref()
                            .unwrap_or(&"unknown".to_string())
                    );

                    ProxyConfig::with_proxy(
                        adspower_proxy.proxy_type.clone(),
                        adspower_proxy.proxy_host.unwrap_or_default(),
                        adspower_proxy.proxy_port.unwrap_or_default(),
                        adspower_proxy.proxy_user,
                        adspower_proxy.proxy_password,
                    )
                }
                None => {
                    warn!("代理池无可用代理，使用无代理配置");
                    ProxyConfig::no_proxy()
                }
            }
        } else {
            warn!("未配置代理池，使用无代理配置");
            ProxyConfig::no_proxy()
        };

        let request = CreateProfileRequest {
            id: None,
            name: Some(username.to_string()),
            group_id: None, // 不指定分组，使用默认分组
            domain_name: Some(config.domain_name.clone()),
            open_urls: Some(config.open_urls.clone()),
            browser_finger_print: BrowserFingerPrint {
                core_version: chrome_version,
                ostype: Some("PC".to_string()),
            },
            ip_check_service: Some("ip123in".to_string()),
            proxy_method: proxy_config.proxy_method,
            proxy_type: proxy_config.proxy_type,
            host: proxy_config.host,
            port: proxy_config.port,
            proxy_user_name: proxy_config.proxy_user_name,
            proxy_password: proxy_config.proxy_password,
            dynamic_ip_url: proxy_config.dynamic_ip_url,
            dynamic_ip_channel: proxy_config.dynamic_ip_channel,
            is_dynamic_ip_change_ip: proxy_config.is_dynamic_ip_change_ip,
        };

        let resp: CreateProfileResponse = self
            .call_api("POST", "/browser/update", Some(request))
            .await?;

        Ok(resp.id)
    }

    pub async fn start_browser(&self, browser_id: &str) -> AppResult<String> {
        let request_body = json!({
            "id": browser_id
        });
        let data: StartBrowserResponse = self
            .call_api("POST", "/browser/open", Some(request_body))
            .await?;

        let ws_url = data.ws;

        info!("BitBrowser 浏览器已启动: {}", ws_url);
        Ok(ws_url)
    }

    pub async fn stop_browser(&self, browser_id: &str) -> AppResult<()> {
        let request_body = json!({
            "id": browser_id
        });
        match self
            .call_api::<serde_json::Value, serde_json::Value>(
                "POST",
                "/browser/close",
                Some(request_body),
            )
            .await
        {
            Ok(_) => {
                info!("{} 的 BitBrowser 浏览器已停止", browser_id);
                Ok(())
            }
            Err(e) => {
                warn!("BitBrowser 停止浏览器错误: {}", e);
                Ok(())
            }
        }
    }

    pub async fn delete_profile(&self, browser_id: &str) -> AppResult<()> {
        let body = json!({
            "id": browser_id
        });

        let _: serde_json::Value = self.call_api("POST", "/browser/delete", Some(body)).await?;

        info!("已删除 BitBrowser 配置文件: {}", browser_id);
        Ok(())
    }
}

#[async_trait]
impl BrowserEnvironmentManager for BitBrowserClient {
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
