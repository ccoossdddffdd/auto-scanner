use std::time::{SystemTime, UNIX_EPOCH};

pub struct FingerprintGenerator;

impl FingerprintGenerator {
    pub fn generate_random_system() -> &'static str {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();

        if nanos % 2 == 0 {
            "Windows"
        } else {
            "Mac"
        }
    }
}
