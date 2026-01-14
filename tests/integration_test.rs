use auto_scanner::services::master::{self, MasterConfig};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_end_to_end_workflow() {
    // 1. Setup Environment
    let test_dir = PathBuf::from("target/test_data");
    let input_dir = test_dir.join("input");
    let doned_dir = test_dir.join("doned");

    // Clean up previous run
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&input_dir).unwrap();
    fs::create_dir_all(&doned_dir).unwrap();

    // 2. Prepare Input File
    let input_file = input_dir.join("test_accounts.csv");
    let content = "username,password\ntest@example.com,password123\n";
    fs::write(&input_file, content).unwrap();

    // 3. Build the binary (we need the real binary for the worker process)
    // We assume the binary is already built by `cargo test` or `cargo build`
    // Usually it is at target/debug/auto-scanner
    let exe_path = PathBuf::from("target/debug/auto-scanner");

    if !exe_path.exists() {
        // Try to build it if not exists
        let status = std::process::Command::new("cargo")
            .arg("build")
            .status()
            .expect("Failed to build binary");
        assert!(status.success(), "Cargo build failed");
    }

    // 4. Configure Master
    env::set_var("DONED_DIR", doned_dir.to_str().unwrap());

    let config = MasterConfig {
        backend: "mock".to_string(),
        remote_url: "".to_string(),
        thread_count: 1,
        enable_screenshot: false,
        stop: false,
        daemon: false,
        status: false,
        enable_email_monitor: false,
        email_poll_interval: 60,
        exe_path: Some(env::current_dir().unwrap().join(exe_path)),
    };

    // 5. Run Master in a separate task
    let input_dir_str = input_dir.to_str().unwrap().to_string();
    let master_handle = tokio::spawn(async move { master::run(Some(input_dir_str), config).await });

    // 6. Wait for result
    let mut success = false;
    for _ in 0..30 {
        // Wait up to 30 seconds
        let entries = fs::read_dir(&doned_dir).unwrap();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("test_accounts.done-") && name.ends_with(".csv") {
                    // Verify content
                    let content = fs::read_to_string(&path).unwrap();
                    println!("Found result file: {}", name);
                    println!("Content:\n{}", content);

                    if content.contains("登录成功") {
                        success = true;
                    }
                    break;
                }
            }
        }
        if success {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    // 7. Cleanup
    master_handle.abort();

    assert!(success, "Failed to process file within timeout");
}
