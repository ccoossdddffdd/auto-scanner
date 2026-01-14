use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "auto-scanner")]
#[command(about = "Automated Facebook account verification tool", long_about = None)]
pub struct Cli {
    /// Path to the CSV file containing account credentials
    #[arg(short, long, value_name = "FILE")]
    pub input: String,

    /// Browser backend to use
    #[arg(long, default_value = "playwright")]
    pub backend: String,

    /// Remote debugging URL for the browser
    #[arg(long, default_value = "http://localhost:9222")]
    pub remote_url: String,

    /// Number of threads to use
    #[arg(long, default_value = "1")]
    pub thread_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_with_input_short() {
        let cli = Cli::try_parse_from(&["auto-scanner", "-i", "accounts.csv"]);
        assert!(cli.is_ok());
        assert_eq!(cli.unwrap().input, "accounts.csv");
    }

    #[test]
    fn test_cli_with_input_long() {
        let cli = Cli::try_parse_from(&["auto-scanner", "--input", "test.csv"]);
        assert!(cli.is_ok());
        assert_eq!(cli.unwrap().input, "test.csv");
    }

    #[test]
    fn test_cli_without_input_should_fail() {
        let cli = Cli::try_parse_from(&["auto-scanner"]);
        assert!(cli.is_err());
    }
}
