use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CreateProfileRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "groupId")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "domainName")]
    pub domain_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "openUrls")]
    pub open_urls: Option<Vec<String>>,
    #[serde(rename = "browserFingerPrint")]
    pub browser_finger_print: BrowserFingerPrint,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ipCheckService")]
    pub ip_check_service: Option<String>,
    #[serde(rename = "proxyMethod")]
    pub proxy_method: i32,
    #[serde(rename = "proxyType")]
    pub proxy_type: String,
    #[serde(skip_serializing_if = "is_empty_string")]
    pub host: String,
    #[serde(skip_serializing_if = "is_empty_string")]
    pub port: String,
    #[serde(skip_serializing_if = "is_empty_string", rename = "proxyUserName")]
    pub proxy_user_name: String,
    #[serde(skip_serializing_if = "is_empty_string", rename = "proxyPassword")]
    pub proxy_password: String,
    #[serde(skip_serializing_if = "is_empty_string", rename = "dynamicIpUrl")]
    pub dynamic_ip_url: String,
    #[serde(skip_serializing_if = "is_empty_string", rename = "dynamicIpChannel")]
    pub dynamic_ip_channel: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "isDynamicIpChangeIp")]
    pub is_dynamic_ip_change_ip: Option<bool>,
}

fn is_empty_string(s: &str) -> bool {
    s.is_empty()
}

#[derive(Debug, Serialize)]
pub struct BrowserFingerPrint {
    #[serde(rename = "coreVersion")]
    pub core_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ostype: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ProxyConfig {
    pub proxy_method: i32,
    pub proxy_type: String,
    pub host: String,
    pub port: String,
    pub proxy_user_name: String,
    pub proxy_password: String,
    pub dynamic_ip_url: String,
    pub dynamic_ip_channel: String,
    pub is_dynamic_ip_change_ip: Option<bool>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            proxy_method: 2,  // noproxy
            proxy_type: "noproxy".to_string(),
            host: String::new(),
            port: String::new(),
            proxy_user_name: String::new(),
            proxy_password: String::new(),
            dynamic_ip_url: String::new(),
            dynamic_ip_channel: String::new(),
            is_dynamic_ip_change_ip: None,
        }
    }
}

impl ProxyConfig {
    /// 创建无代理配置
    pub fn no_proxy() -> Self {
        Self::default()
    }

    /// 创建动态代理配置（API 提取链接）
    pub fn with_dynamic_ip(dynamic_ip_url: String) -> Self {
        Self {
            proxy_method: 3,  // API 提取
            proxy_type: "socks5".to_string(),
            host: String::new(),
            port: String::new(),
            proxy_user_name: String::new(),
            proxy_password: String::new(),
            dynamic_ip_url,
            dynamic_ip_channel: "common".to_string(),
            is_dynamic_ip_change_ip: Some(true),
        }
    }

    /// 创建静态代理配置
    pub fn with_proxy(
        proxy_type: String,
        host: String,
        port: String,
        user_name: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            proxy_method: 2,  // extract_proxy
            proxy_type,
            host,
            port,
            proxy_user_name: user_name.unwrap_or_default(),
            proxy_password: password.unwrap_or_default(),
            dynamic_ip_url: String::new(),
            dynamic_ip_channel: String::new(),
            is_dynamic_ip_change_ip: None,
        }
    }
}
