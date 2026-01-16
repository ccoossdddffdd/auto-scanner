use fake::faker::name::raw::*;
use fake::locales::*;
use fake::Fake;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub first_name: String,
    pub last_name: String,
    pub password: String,
    pub email_username: String,
    pub birth_day: u32,
    pub birth_month: u32,
    pub birth_year: u32,
}

pub struct UserInfoGenerator;

impl UserInfoGenerator {
    pub fn generate() -> UserInfo {
        let first_name: String = FirstName(EN).fake();
        let last_name: String = LastName(EN).fake();

        let mut rng = rand::rng();
        let random_suffix: u32 = rng.random_range(1000..99999);
        let email_username = format!(
            "{}{}{}",
            first_name.to_lowercase(),
            last_name.to_lowercase(),
            random_suffix
        );

        let password = Self::generate_secure_password();

        // Generate valid birth date (18+ years old)
        let current_year = chrono::Local::now().year();
        let birth_year = rng.random_range((current_year - 50)..(current_year - 18)) as u32;
        let birth_month = rng.random_range(1..=12);
        let birth_day = rng.random_range(1..=28); // Safe for all months

        UserInfo {
            first_name,
            last_name,
            password,
            email_username,
            birth_day,
            birth_month,
            birth_year,
        }
    }

    fn generate_secure_password() -> String {
        use rand::distr::Alphanumeric;
        let mut rng = rand::rng();

        // Ensure at least one uppercase, one lowercase, one number, one special char
        let password: String = std::iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(12)
            .collect();

        // Add required complexity if needed, but 12 random alphanumeric is usually fine.
        // Let's make it stronger by appending specific chars to guarantee requirements
        format!("{}A1!", password)
    }
}

use chrono::Datelike;
