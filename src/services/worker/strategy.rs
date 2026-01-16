use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerStrategy {
    FacebookLogin,
}

impl FromStr for WorkerStrategy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "facebook_login" => Ok(WorkerStrategy::FacebookLogin),
            _ => Err(anyhow::anyhow!("Unsupported strategy: {}", s)),
        }
    }
}

impl ToString for WorkerStrategy {
    fn to_string(&self) -> String {
        match self {
            WorkerStrategy::FacebookLogin => "facebook_login".to_string(),
        }
    }
}
