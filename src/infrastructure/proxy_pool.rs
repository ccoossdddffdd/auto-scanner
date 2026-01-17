use crate::core::error::{AppError, AppResult};
use crate::infrastructure::adspower::types::UserProxyConfig;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfig {
    pub host: String,
    pub port: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub refresh_url: Option<String>,
}

impl ProxyConfig {
    pub fn to_user_proxy_config(&self) -> UserProxyConfig {
        UserProxyConfig::with_proxy(
            self.proxy_type.clone(),
            self.host.clone(),
            self.port.clone(),
            self.username.clone(),
            self.password.clone(),
            self.refresh_url.clone(),
        )
    }

    /// 生成代理标识符（用于黑名单管理）
    pub fn identifier(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone)]
pub enum ProxyStrategy {
    RoundRobin,
    Random,
    Sticky, // 每个 Worker 固定一个代理
}

pub struct ProxyPoolManager {
    proxies: Vec<ProxyConfig>,
    index: Arc<AtomicUsize>,
    blacklist: Arc<RwLock<HashSet<String>>>,
    strategy: ProxyStrategy,
}

impl ProxyPoolManager {
    /// 从 CSV 文件加载代理池
    pub fn from_csv<P: AsRef<Path>>(path: P) -> AppResult<Self> {
        let path = path.as_ref();
        info!("正在从 {} 加载代理池配置...", path.display());

        if !path.exists() {
            return Err(AppError::Config(format!(
                "代理池配置文件不存在: {}",
                path.display()
            )));
        }

        let mut reader = csv::Reader::from_path(path).map_err(|e| {
            AppError::Parse(format!("无法读取代理池 CSV 文件: {}", e))
        })?;

        let mut proxies = Vec::new();
        for result in reader.deserialize() {
            let proxy: ProxyConfig = result.map_err(|e| {
                AppError::Parse(format!("解析代理配置失败: {}", e))
            })?;
            proxies.push(proxy);
        }

        if proxies.is_empty() {
            return Err(AppError::Config(
                "代理池配置文件为空，至少需要一个代理".to_string(),
            ));
        }

        info!("成功加载 {} 个代理配置", proxies.len());

        Ok(Self {
            proxies,
            index: Arc::new(AtomicUsize::new(0)),
            blacklist: Arc::new(RwLock::new(HashSet::new())),
            strategy: ProxyStrategy::RoundRobin,
        })
    }

    /// 设置代理分配策略
    pub fn with_strategy(mut self, strategy: ProxyStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// 获取下一个可用代理（根据策略）
    pub async fn get_next(&self) -> Option<UserProxyConfig> {
        let blacklist = self.blacklist.read().await;

        match self.strategy {
            ProxyStrategy::RoundRobin => self.get_round_robin(&blacklist).await,
            ProxyStrategy::Random => self.get_random(&blacklist).await,
            ProxyStrategy::Sticky => self.get_round_robin(&blacklist).await, // Sticky 由外部控制
        }
    }

    /// 为指定的 Worker 索引获取固定代理（粘性分配）
    pub async fn get_for_worker(&self, worker_index: usize) -> Option<UserProxyConfig> {
        let blacklist = self.blacklist.read().await;
        let available_proxies: Vec<_> = self
            .proxies
            .iter()
            .filter(|p| !blacklist.contains(&p.identifier()))
            .collect();

        if available_proxies.is_empty() {
            warn!("所有代理均不可用（黑名单）");
            return None;
        }

        // 粘性分配：Worker 索引对应代理索引
        let proxy = available_proxies[worker_index % available_proxies.len()];
        Some(proxy.to_user_proxy_config())
    }

    /// 轮询策略
    async fn get_round_robin(&self, blacklist: &HashSet<String>) -> Option<UserProxyConfig> {
        let available_proxies: Vec<_> = self
            .proxies
            .iter()
            .filter(|p| !blacklist.contains(&p.identifier()))
            .collect();

        if available_proxies.is_empty() {
            warn!("所有代理均不可用（黑名单）");
            return None;
        }

        let idx = self.index.fetch_add(1, Ordering::SeqCst) % available_proxies.len();
        Some(available_proxies[idx].to_user_proxy_config())
    }

    /// 随机策略
    async fn get_random(&self, blacklist: &HashSet<String>) -> Option<UserProxyConfig> {
        use rand::prelude::IndexedRandom;
        
        let available_proxies: Vec<_> = self
            .proxies
            .iter()
            .filter(|p| !blacklist.contains(&p.identifier()))
            .collect();

        if available_proxies.is_empty() {
            warn!("所有代理均不可用（黑名单）");
            return None;
        }

        let proxy = available_proxies
            .choose(&mut rand::rng())
            .expect("已验证非空");
        Some(proxy.to_user_proxy_config())
    }

    /// 标记代理为失效（加入黑名单）
    pub async fn mark_failed(&self, host: &str, port: &str) {
        let identifier = format!("{}:{}", host, port);
        let mut blacklist = self.blacklist.write().await;
        
        if blacklist.insert(identifier.clone()) {
            warn!("代理 {} 已标记为失效并加入黑名单", identifier);
        }
    }

    /// 清除黑名单（重置所有代理状态）
    pub async fn clear_blacklist(&self) {
        let mut blacklist = self.blacklist.write().await;
        let count = blacklist.len();
        blacklist.clear();
        info!("已清除代理黑名单（共 {} 条记录）", count);
    }

    /// 获取可用代理数量
    pub async fn available_count(&self) -> usize {
        let blacklist = self.blacklist.read().await;
        self.proxies
            .iter()
            .filter(|p| !blacklist.contains(&p.identifier()))
            .count()
    }

    /// 获取总代理数量
    pub fn total_count(&self) -> usize {
        self.proxies.len()
    }

    /// 健康检查（验证代理可用性）
    pub async fn health_check(&self) -> AppResult<()> {
        info!("开始执行代理池健康检查...");

        let mut failed_proxies = Vec::new();

        for proxy in &self.proxies {
            let proxy_url = format!(
                "{}://{}:{}",
                proxy.proxy_type, proxy.host, proxy.port
            );

            let mut proxy_builder = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| AppError::Network(format!("构建代理失败: {}", e)))?;

            if let (Some(user), Some(pass)) = (&proxy.username, &proxy.password) {
                proxy_builder = proxy_builder.basic_auth(user, pass);
            }

            let test_client = reqwest::Client::builder()
                .proxy(proxy_builder)
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| AppError::Network(format!("创建代理客户端失败: {}", e)))?;

            match test_client.get("https://ipinfo.io/json").send().await {
                Ok(resp) if resp.status().is_success() => {
                    info!("代理 {} 健康检查通过", proxy.identifier());
                }
                Ok(resp) => {
                    warn!(
                        "代理 {} 返回异常状态码: {}",
                        proxy.identifier(),
                        resp.status()
                    );
                    failed_proxies.push(proxy.identifier());
                }
                Err(e) => {
                    warn!("代理 {} 健康检查失败: {}", proxy.identifier(), e);
                    failed_proxies.push(proxy.identifier());
                }
            }
        }

        // 将失败的代理加入黑名单
        if !failed_proxies.is_empty() {
            let mut blacklist = self.blacklist.write().await;
            for identifier in &failed_proxies {
                blacklist.insert(identifier.clone());
            }
            warn!(
                "健康检查完成，{}/{} 个代理失败并已加入黑名单",
                failed_proxies.len(),
                self.proxies.len()
            );
        } else {
            info!("健康检查完成，所有代理状态正常");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proxy_pool_round_robin() {
        // 创建测试数据
        let proxies = vec![
            ProxyConfig {
                host: "192.168.1.1".to_string(),
                port: "8080".to_string(),
                proxy_type: "http".to_string(),
                username: None,
                password: None,
                refresh_url: None,
            },
            ProxyConfig {
                host: "192.168.1.2".to_string(),
                port: "8080".to_string(),
                proxy_type: "http".to_string(),
                username: None,
                password: None,
                refresh_url: None,
            },
        ];

        let manager = ProxyPoolManager {
            proxies,
            index: Arc::new(AtomicUsize::new(0)),
            blacklist: Arc::new(RwLock::new(HashSet::new())),
            strategy: ProxyStrategy::RoundRobin,
        };

        // 测试轮询
        let proxy1 = manager.get_next().await.unwrap();
        let proxy2 = manager.get_next().await.unwrap();
        let proxy3 = manager.get_next().await.unwrap();

        assert_eq!(proxy1.proxy_host.as_ref().unwrap(), "192.168.1.1");
        assert_eq!(proxy2.proxy_host.as_ref().unwrap(), "192.168.1.2");
        assert_eq!(proxy3.proxy_host.as_ref().unwrap(), "192.168.1.1"); // 回到第一个
    }

    #[tokio::test]
    async fn test_blacklist() {
        let proxies = vec![ProxyConfig {
            host: "192.168.1.1".to_string(),
            port: "8080".to_string(),
            proxy_type: "http".to_string(),
            username: None,
            password: None,
            refresh_url: None,
        }];

        let manager = ProxyPoolManager {
            proxies,
            index: Arc::new(AtomicUsize::new(0)),
            blacklist: Arc::new(RwLock::new(HashSet::new())),
            strategy: ProxyStrategy::RoundRobin,
        };

        // 标记为失效
        manager.mark_failed("192.168.1.1", "8080").await;

        // 应该无法获取代理
        let proxy = manager.get_next().await;
        assert!(proxy.is_none());

        // 清除黑名单
        manager.clear_blacklist().await;
        let proxy = manager.get_next().await;
        assert!(proxy.is_some());
    }
}
