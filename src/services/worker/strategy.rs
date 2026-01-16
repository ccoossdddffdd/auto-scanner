use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStrategy {
    FacebookLogin,
    OutlookRegister,
}

impl FromStr for WorkerStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "facebook_login" => Ok(WorkerStrategy::FacebookLogin),
            "outlook_register" => Ok(WorkerStrategy::OutlookRegister),
            _ => Err(anyhow::anyhow!("Unsupported strategy: {}", s)),
        }
    }
}

impl ToString for WorkerStrategy {
    fn to_string(&self) -> String {
        match self {
            WorkerStrategy::FacebookLogin => "facebook_login".to_string(),
            WorkerStrategy::OutlookRegister => "outlook_register".to_string(),
        }
    }
}
