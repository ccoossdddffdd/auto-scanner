use auto_scanner::infrastructure::bitbrowser::{BitBrowserClient, BitBrowserConfig};
use auto_scanner::infrastructure::browser_manager::BrowserEnvironmentManager;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_bitbrowser_connectivity() {
    // Load .env file
    dotenv::dotenv().ok();

    // 1. 初始化配置 (尝试从环境变量读取，否则使用默认)
    let config = BitBrowserConfig::from_env().expect("Invalid config");

    let client = BitBrowserClient::new(config).expect("Failed to create client");

    // 2. 检查连接性 (如果本地没有运行 BitBrowser，这会失败)
    let result = client.check_connectivity().await;

    if result.is_err() {
        eprintln!(
            "Skipping test: BitBrowser service not reachable. Error: {:?}",
            result.err()
        );
        return;
    }

    assert!(result.is_ok(), "Connectivity check failed");
}

#[tokio::test]
async fn test_bitbrowser_complete_flow() {
    // Load .env file
    dotenv::dotenv().ok();

    // 1. 初始化
    let config = BitBrowserConfig::from_env().expect("Invalid config");
    let client = BitBrowserClient::new(config).expect("Failed to create client");

    // 先检查服务是否可用，不可用则跳过测试
    if client.check_connectivity().await.is_err() {
        eprintln!("Skipping complete flow test: BitBrowser service not reachable");
        return;
    }

    // 使用一个特殊的 thread_index 以避免与正常运行的 worker 冲突
    let test_thread_index = 9999;
    let _profile_name = format!("auto-scanner-worker-{}", test_thread_index);

    // 2. 确保没有残留的测试配置文件 (清理环境)
    // 我们先尝试查找，如果找到了就删除
    // 注意：BitBrowserClient 没有公开 find_profile_by_name，但我们可以通过 ensure_profile_for_thread 来获取 ID
    // 或者我们假设 ensure_profile_for_thread 会正确处理已存在的情况

    // 3. 创建/获取配置文件
    println!("Creating profile for thread {}...", test_thread_index);
    let profile_id_result = client
        .ensure_profile_for_thread(test_thread_index, None)
        .await;
    assert!(
        profile_id_result.is_ok(),
        "Failed to create profile: {:?}",
        profile_id_result.err()
    );
    let profile_id = profile_id_result.unwrap();
    println!("Profile created/found. ID: {}", profile_id);
    assert!(!profile_id.is_empty(), "Profile ID should not be empty");

    // 4. 启动浏览器
    println!("Starting browser {}...", profile_id);
    let start_result = client.start_browser(&profile_id).await;
    assert!(
        start_result.is_ok(),
        "Failed to start browser: {:?}",
        start_result.err()
    );
    let ws_url = start_result.unwrap();
    println!("Browser started. WS URL: {}", ws_url);
    assert!(ws_url.starts_with("ws://"), "Invalid WS URL: {}", ws_url);

    // 5. 等待几秒，模拟浏览器运行
    sleep(Duration::from_secs(5)).await;

    // 6. 停止浏览器
    println!("Stopping browser {}...", profile_id);
    let stop_result = client.stop_browser(&profile_id).await;
    assert!(
        stop_result.is_ok(),
        "Failed to stop browser: {:?}",
        stop_result.err()
    );
    println!("Browser stopped.");

    // 7. 删除配置文件 (清理资源)
    println!("Deleting profile {}...", profile_id);
    let delete_result = client.delete_profile(&profile_id).await;
    assert!(
        delete_result.is_ok(),
        "Failed to delete profile: {:?}",
        delete_result.err()
    );
    println!("Profile deleted.");
}
