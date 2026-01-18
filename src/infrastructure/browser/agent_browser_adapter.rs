use super::{BrowserAdapter, BrowserCookie, BrowserError};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Agent Browser CLI 适配器
/// 使用 agent-browser CLI 工具进行浏览器自动化
pub struct AgentBrowserAdapter {
    session: String,
    executable: String,
}

impl AgentBrowserAdapter {
    /// 创建新的 Agent Browser 适配器
    pub async fn new(session_name: Option<String>) -> Result<Self, BrowserError> {
        let session = session_name.unwrap_or_else(|| format!("auto-scanner-{}", uuid::Uuid::new_v4()));
        let executable = std::env::var("AGENT_BROWSER_PATH")
            .unwrap_or_else(|_| "agent-browser".to_string());

        info!("初始化 Agent Browser 适配器，会话: {}", session);

        // 检查 agent-browser 是否可用
        let output = Command::new(&executable)
            .arg("--version")
            .output()
            .await;

        if output.is_err() {
            return Err(BrowserError::ConnectionFailed(
                "agent-browser 未安装或不在 PATH 中。请运行: npm install -g agent-browser".to_string(),
            ));
        }

        Ok(Self {
            session,
            executable,
        })
    }

    /// 执行 agent-browser 命令
    async fn exec(&self, args: &[&str]) -> Result<String, BrowserError> {
        let mut cmd = Command::new(&self.executable);
        cmd.arg("--session").arg(&self.session).arg("--json");

        for arg in args {
            cmd.arg(arg);
        }

        debug!("执行命令: {:?}", cmd);

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| BrowserError::Other(format!("执行命令失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("命令执行失败: {}", stderr);
            return Err(BrowserError::Other(format!("命令失败: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        debug!("命令输出: {}", stdout);
        Ok(stdout)
    }

    /// 执行命令并解析 JSON 响应
    async fn exec_json(&self, args: &[&str]) -> Result<Value, BrowserError> {
        let output = self.exec(args).await?;
        serde_json::from_str(&output)
            .map_err(|e| BrowserError::Other(format!("解析 JSON 失败: {}", e)))
    }

    /// 关闭浏览器会话
    pub async fn close_session(&self) -> Result<(), BrowserError> {
        info!("关闭 Agent Browser 会话: {}", self.session);
        self.exec(&["close"]).await?;
        Ok(())
    }
}

#[async_trait]
impl BrowserAdapter for AgentBrowserAdapter {
    async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        info!("导航到: {}", url);
        self.exec(&["open", url]).await?;
        Ok(())
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        debug!("在 {} 中输入文本", selector);
        self.exec(&["type", selector, text]).await?;
        Ok(())
    }

    async fn click(&self, selector: &str) -> Result<(), BrowserError> {
        debug!("点击元素: {}", selector);
        self.exec(&["click", selector]).await?;
        Ok(())
    }

    async fn wait_for_element(&self, selector: &str) -> Result<(), BrowserError> {
        debug!("等待元素: {}", selector);
        self.exec(&["wait", selector]).await?;
        Ok(())
    }

    async fn is_visible(&self, selector: &str) -> Result<bool, BrowserError> {
        let result = self.exec_json(&["is", "visible", selector]).await?;
        
        Ok(result.get("visible").and_then(|v| v.as_bool()).unwrap_or(false))
    }

    async fn get_cookies(&self) -> Result<Vec<BrowserCookie>, BrowserError> {
        let cookies = self.exec_json(&["cookies"]).await?;

        let mut result = Vec::new();
        if let Some(arr) = cookies.as_array() {
            for cookie in arr {
                if let Some(obj) = cookie.as_object() {
                    result.push(BrowserCookie {
                        name: obj.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        value: obj.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        domain: obj.get("domain").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        path: obj.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
                        expires: obj.get("expires").and_then(|v| v.as_f64()),
                        http_only: obj.get("httpOnly").and_then(|v| v.as_bool()),
                        secure: obj.get("secure").and_then(|v| v.as_bool()),
                        same_site: obj.get("sameSite").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    });
                }
            }
        }

        Ok(result)
    }

    async fn set_cookies(&self, cookies: &[BrowserCookie]) -> Result<(), BrowserError> {
        for cookie in cookies {
            self.exec(&["cookies", "set", &cookie.name, &cookie.value]).await?;
        }
        Ok(())
    }

    async fn take_screenshot(&self, path: &str) -> Result<(), BrowserError> {
        info!("截图保存到: {}", path);
        self.exec(&["screenshot", path]).await?;
        Ok(())
    }

    async fn get_current_url(&self) -> Result<String, BrowserError> {
        let result = self.exec_json(&["get", "url"]).await?;
        
        result.get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| BrowserError::Other("无法获取 URL".to_string()))
    }

    async fn get_text(&self, selector: &str) -> Result<String, BrowserError> {
        let result = self.exec_json(&["get", "text", selector]).await?;
        
        result.get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| BrowserError::ElementNotFound(selector.to_string()))
    }

    async fn get_all_text(&self, selector: &str) -> Result<Vec<String>, BrowserError> {
        // agent-browser 不直接支持获取所有匹配元素的文本
        // 这里先获取数量，然后逐个获取
        let count_result = self.exec_json(&["get", "count", selector]).await?;
        
        let count = count_result.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let mut texts = Vec::new();
        for i in 0..count {
            let nth_selector = format!("{}:nth-of-type({})", selector, i + 1);
            if let Ok(text) = self.get_text(&nth_selector).await {
                texts.push(text);
            }
        }

        Ok(texts)
    }

    async fn select_option(&self, selector: &str, value: &str) -> Result<(), BrowserError> {
        debug!("选择选项 {} 在 {}", value, selector);
        self.exec(&["select", selector, value]).await?;
        Ok(())
    }

    async fn get_content(&self) -> Result<String, BrowserError> {
        let result = self.exec_json(&["get", "html", "body"]).await?;
        
        result.get("html")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| BrowserError::Other("无法获取页面内容".to_string()))
    }
}

impl Drop for AgentBrowserAdapter {
    fn drop(&mut self) {
        // 异步关闭会话（最佳努力）
        info!("AgentBrowserAdapter 正在销毁");
    }
}
