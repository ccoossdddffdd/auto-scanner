use std::time::{SystemTime, UNIX_EPOCH};

pub struct FingerprintGenerator;

impl FingerprintGenerator {
    pub fn generate_random_system() -> &'static str {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();

        if nanos.is_multiple_of(2) {
            "Windows"
        } else {
            "Mac"
        }
    }
}
