use crate::infrastructure::adspower::ProfileConfig;

pub trait StrategyProfileProvider: Send + Sync {
    fn get_profile_config(&self, strategy: &str) -> Option<ProfileConfig>;
}

pub struct DefaultStrategyProfileProvider;

impl StrategyProfileProvider for DefaultStrategyProfileProvider {
    fn get_profile_config(&self, strategy: &str) -> Option<ProfileConfig> {
        match strategy {
            "facebook_login" => Some(crate::strategies::facebook_login::get_profile_config()),
            "outlook_register" => Some(crate::strategies::outlook_register::get_profile_config()),
            _ => None,
        }
    }
}
