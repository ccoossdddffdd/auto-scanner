use crate::core::models::WorkerResult;
use crate::strategies::facebook_login::LoginStatus;
use tracing::info;

pub struct FacebookResultBuilder;

impl FacebookResultBuilder {
    pub fn build(status: LoginStatus, friends_count: Option<u32>) -> WorkerResult {
        let mut data = serde_json::Map::new();
        data.insert(
            "验证码".to_string(),
            serde_json::Value::String("不需要".to_string()),
        );
        data.insert(
            "2FA".to_string(),
            serde_json::Value::String("不需要".to_string()),
        );

        let mut result = WorkerResult {
            status: "登录失败".to_string(),
            message: "未知失败".to_string(),
            data: Some(data),
        };

        match status {
            LoginStatus::Success => {
                info!("Login detected as successful");
                result.status = "登录成功".to_string();
                result.message = "成功".to_string();

                if let Some(count) = friends_count {
                    if let Some(data) = &mut result.data {
                        data.insert(
                            "好友数量".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(count)),
                        );
                    }
                    info!("Friends count: {}", count);
                }
            }
            LoginStatus::Captcha => {
                info!("Captcha detected");
                if let Some(data) = &mut result.data {
                    data.insert(
                        "验证码".to_string(),
                        serde_json::Value::String("需要".to_string()),
                    );
                }
                result.message = "检测到验证码".to_string();
            }
            LoginStatus::TwoFactor => {
                info!("2FA detected");
                if let Some(data) = &mut result.data {
                    data.insert(
                        "2FA".to_string(),
                        serde_json::Value::String("需要".to_string()),
                    );
                }
                result.message = "检测到 2FA".to_string();
            }
            LoginStatus::AccountLocked => {
                info!("Account locked detected");
                result.status = "登录失败".to_string();
                result.message = "账号已锁定".to_string();
            }
            LoginStatus::WrongPassword => {
                info!("Wrong password detected");
                result.status = "登录失败".to_string();
                result.message = "密码错误".to_string();
            }
            LoginStatus::Failed => {}
        }

        result
    }
}
