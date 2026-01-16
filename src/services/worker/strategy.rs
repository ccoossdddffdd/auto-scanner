use std::str::FromStr;

use std::fmt;

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

impl fmt::Display for WorkerStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerStrategy::FacebookLogin => write!(f, "facebook_login"),
            WorkerStrategy::OutlookRegister => write!(f, "outlook_register"),
        }
    }
}
