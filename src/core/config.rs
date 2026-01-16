use crate::infrastructure::adspower::AdsPowerConfig;
use crate::services::email::EmailConfig;
use crate::services::master::MasterConfig;
use anyhow::{Context, Result};
use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub master: MasterConfig,
    pub adspower: Option<AdsPowerConfig>,
    pub email: Option<EmailConfig>,
    pub input_dir: String,
}

impl AppConfig {
    pub fn new(master: MasterConfig) -> Result<Self> {
        dotenv::dotenv().ok();

        // Check essential env vars
        let input_dir = env::var("INPUT_DIR").context("必须设置 INPUT_DIR 环境变量")?;

        let adspower = if master.backend == "adspower" {
            Some(AdsPowerConfig::from_env()?)
        } else {
            None
        };

        let email = if master.enable_email_monitor {
            match EmailConfig::from_env() {
                Ok(c) => Some(c),
                Err(e) => {
                    // Log handled by caller or just ignored here?
                    // Since this is a constructor, maybe we shouldn't log here unless we have logger initialized.
                    // Logger is initialized in main before this.
                    tracing::warn!("创建邮件配置失败: {}, 禁用邮件监控", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            master,
            adspower,
            email,
            input_dir,
        })
    }
}
