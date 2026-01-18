use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CreateProfileRequest {
    pub name: String,
    pub group_id: String,
    pub domain_name: String,
    pub open_urls: Vec<String>,
    pub fingerprint_config: FingerprintConfig,
    pub user_proxy_config: UserProxyConfig,
    pub proxyid: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateProfileRequest {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint_config: Option<FingerprintConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_proxy_config: Option<UserProxyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxyid: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FingerprintConfig {
    pub random_ua: RandomUaConfig,
}

#[derive(Debug, Serialize)]
pub struct RandomUaConfig {
    pub ua_browser: Vec<String>,
    pub ua_system_version: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserProxyConfig {
    pub proxy_soft: String,
    pub proxy_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_change_ip_url: Option<String>,
}

impl Default for UserProxyConfig {
    fn default() -> Self {
        Self {
            proxy_soft: "other".to_string(),
            proxy_type: "noproxy".to_string(),
            proxy_host: None,
            proxy_port: None,
            proxy_user: None,
            proxy_password: None,
            proxy_change_ip_url: None,
        }
    }
}

impl UserProxyConfig {
    /// 创建使用 AdsPower 代理池 ID 的配置（proxyid 优先）
    pub fn with_proxyid() -> Self {
        Self::default()
    }

    /// 创建动态代理配置
    pub fn with_proxy(
        proxy_type: String,
        host: String,
        port: String,
        user: Option<String>,
        password: Option<String>,
        refresh_url: Option<String>,
    ) -> Self {
        Self {
            proxy_soft: "ads_power".to_string(),
            proxy_type,
            proxy_host: Some(host),
            proxy_port: Some(port),
            proxy_user: user,
            proxy_password: password,
            proxy_change_ip_url: refresh_url,
        }
    }
}
