use crate::services::worker::strategy::WorkerStrategy;
use crate::strategies::{
    facebook_login::FacebookLoginStrategy, outlook_register::OutlookRegisterStrategy, BaseStrategy,
};
use anyhow::Result;

pub struct StrategyFactory;

impl StrategyFactory {
    pub fn create(strategy: WorkerStrategy) -> Result<Box<dyn BaseStrategy>> {
        match strategy {
            WorkerStrategy::FacebookLogin => Ok(Box::new(FacebookLoginStrategy::new())),
            WorkerStrategy::OutlookRegister => Ok(Box::new(OutlookRegisterStrategy::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_facebook_strategy() {
        let strategy = StrategyFactory::create(WorkerStrategy::FacebookLogin);
        assert!(strategy.is_ok());
    }
}
