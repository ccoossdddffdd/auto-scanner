use auto_scanner::core::models::Account;
use auto_scanner::infrastructure::browser::mock_adapter::MockBrowserAdapter;
use auto_scanner::strategies::outlook_register::OutlookRegisterStrategy;
use auto_scanner::strategies::BaseStrategy;
use chrono::Datelike;

#[tokio::test]
async fn test_outlook_register_complete_flow() {
    // Arrange
    let strategy = OutlookRegisterStrategy::new();
    let adapter = MockBrowserAdapter::new();
    let account = Account::new("test_user".to_string(), "test_password".to_string());

    // Act
    let result = strategy.run(&adapter, &account).await;

    // Assert
    assert!(result.is_ok(), "Strategy execution failed: {:?}", result);

    let worker_result = result.unwrap();

    // 验证状态
    assert_eq!(worker_result.status, "处理中");
    assert!(worker_result.message.contains("已填写基础信息"));

    // 验证返回的数据
    assert!(worker_result.data.is_some(), "Data should not be None");

    let data = worker_result.data.unwrap();

    // 验证必需字段存在
    assert!(data.contains_key("email"), "Email field missing");
    assert!(data.contains_key("password"), "Password field missing");
    assert!(data.contains_key("first_name"), "First name field missing");
    assert!(data.contains_key("last_name"), "Last name field missing");
    assert!(data.contains_key("birth_year"), "Birth year field missing");

    // 验证邮箱格式
    if let Some(email) = data.get("email") {
        let email_str = email.as_str().unwrap();
        assert!(
            email_str.ends_with("@outlook.com"),
            "Email should end with @outlook.com"
        );
        assert!(!email_str.starts_with("@"), "Email should have a username");
    }

    // 验证密码不为空且包含复杂字符
    if let Some(password) = data.get("password") {
        let password_str = password.as_str().unwrap();
        assert!(!password_str.is_empty(), "Password should not be empty");
        assert!(
            password_str.len() >= 12,
            "Password should be at least 12 characters"
        );
    }

    // 验证出生年份合理（18-50岁）
    if let Some(birth_year) = data.get("birth_year") {
        let year = birth_year.as_u64().unwrap() as i32;
        let current_year = chrono::Local::now().year();
        let age = current_year - year;
        assert!(
            (18..=50).contains(&age),
            "Age should be between 18 and 50, got age: {}",
            age
        );
    }

    println!("✅ Outlook register test passed successfully!");
    println!(
        "📧 Generated email: {}",
        data.get("email").unwrap().as_str().unwrap()
    );
}

#[tokio::test]
async fn test_outlook_register_user_info_generation() {
    use auto_scanner::strategies::outlook_register::generator::UserInfoGenerator;

    // 生成10个用户信息，验证它们都是有效的
    for i in 0..10 {
        let user_info = UserInfoGenerator::generate();

        // 验证姓名不为空
        assert!(
            !user_info.first_name.is_empty(),
            "First name should not be empty"
        );
        assert!(
            !user_info.last_name.is_empty(),
            "Last name should not be empty"
        );

        // 验证邮箱用户名
        assert!(
            !user_info.email_username.is_empty(),
            "Email username should not be empty"
        );
        assert!(user_info
            .email_username
            .contains(&user_info.first_name.to_lowercase()));
        assert!(user_info
            .email_username
            .contains(&user_info.last_name.to_lowercase()));

        // 验证密码复杂度
        assert!(
            user_info.password.len() >= 12,
            "Password should be at least 12 characters"
        );
        assert!(
            user_info.password.chars().any(|c| c.is_uppercase()),
            "Password should contain uppercase"
        );
        assert!(
            user_info.password.chars().any(|c| c.is_numeric()),
            "Password should contain numbers"
        );
        assert!(
            user_info.password.chars().any(|c| !c.is_alphanumeric()),
            "Password should contain special chars"
        );

        // 验证生日日期范围
        assert!(
            user_info.birth_month >= 1 && user_info.birth_month <= 12,
            "Month should be 1-12"
        );
        assert!(
            user_info.birth_day >= 1 && user_info.birth_day <= 28,
            "Day should be 1-28"
        );

        let current_year = chrono::Local::now().year();
        let age = current_year - user_info.birth_year as i32;
        assert!((18..=50).contains(&age), "Age should be between 18 and 50");

        println!(
            "✅ Test {}: {} {} - {}@outlook.com",
            i + 1,
            user_info.first_name,
            user_info.last_name,
            user_info.email_username
        );
    }

    println!("✅ All 10 user info generations are valid!");
}

#[test]
fn test_outlook_register_constants() {
    use auto_scanner::strategies::outlook_register::constants::*;

    // 验证选择器配置存在且不为空
    assert!(
        !NEXT_BUTTON_SELECTORS.is_empty(),
        "Next button selectors should not be empty"
    );
    assert!(
        !AGREE_BUTTON_SELECTORS.is_empty(),
        "Agree button selectors should not be empty"
    );
    assert!(
        !BIRTH_YEAR_SELECTORS.is_empty(),
        "Birth year selectors should not be empty"
    );
    assert!(
        !BIRTH_MONTH_SELECTORS.is_empty(),
        "Birth month selectors should not be empty"
    );
    assert!(
        !BIRTH_DAY_SELECTORS.is_empty(),
        "Birth day selectors should not be empty"
    );
    assert!(
        !FIRST_NAME_SELECTORS.is_empty(),
        "First name selectors should not be empty"
    );
    assert!(
        !LAST_NAME_SELECTORS.is_empty(),
        "Last name selectors should not be empty"
    );
    assert!(!BOT_KEYWORDS.is_empty(), "Bot keywords should not be empty");

    // 验证多语言支持
    assert!(
        NEXT_BUTTON_SELECTORS.iter().any(|s| s.contains("Next")),
        "Should have English"
    );
    assert!(
        NEXT_BUTTON_SELECTORS.iter().any(|s| s.contains("下一步")),
        "Should have Chinese"
    );
    assert!(
        NEXT_BUTTON_SELECTORS.iter().any(|s| s.contains("次へ")),
        "Should have Japanese"
    );

    // 验证月份名称函数
    let january_names = get_month_names(1);
    assert!(
        !january_names.is_empty(),
        "January names should not be empty"
    );
    assert!(
        january_names.contains(&"January"),
        "Should contain 'January'"
    );
    assert!(
        january_names.contains(&"一月"),
        "Should contain Chinese '一月'"
    );

    println!("✅ All constants are properly configured!");
}

#[test]
fn test_outlook_register_profile_config() {
    use auto_scanner::strategies::outlook_register::get_profile_config;

    let config = get_profile_config();

    // 验证配置
    assert_eq!(
        config.domain_name, "outlook.com",
        "Domain should be outlook.com"
    );
    assert_eq!(config.group_id, "0", "Group ID should be '0'");
    assert!(
        !config.open_urls.is_empty(),
        "Open URLs should not be empty"
    );
    assert_eq!(
        config.open_urls[0], "https://signup.live.com/",
        "First URL should be signup page"
    );

    println!("✅ Profile config is correct!");
}
