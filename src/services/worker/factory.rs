use crate::strategies::{facebook_login::FacebookLoginStrategy, BaseStrategy};
use anyhow::Result;

pub struct StrategyFactory;

impl StrategyFactory {
    pub fn create(strategy_name: &str) -> Result<Box<dyn BaseStrategy>> {
        match strategy_name {
            "facebook_login" => Ok(Box::new(FacebookLoginStrategy::new())),
            _ => Err(anyhow::anyhow!("不支持的策略: {}", strategy_name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_facebook_strategy() {
        let strategy = StrategyFactory::create("facebook_login");
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_create_unknown_strategy() {
        let strategy = StrategyFactory::create("unknown_strategy");
        assert!(strategy.is_err());
    }
}
