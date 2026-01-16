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
pub struct FingerprintConfig {
    pub random_ua: RandomUaConfig,
}

#[derive(Debug, Serialize)]
pub struct RandomUaConfig {
    pub ua_browser: Vec<String>,
    pub ua_system_version: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UserProxyConfig {
    pub proxy_soft: String,
    pub proxy_type: String,
}

impl Default for UserProxyConfig {
    fn default() -> Self {
        Self {
            proxy_soft: "other".to_string(),
            proxy_type: "noproxy".to_string(),
        }
    }
}
