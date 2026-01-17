use rand::Rng;

pub struct FingerprintGenerator;

impl FingerprintGenerator {
    /// 生成随机 Chrome 版本（在 140 和 142 之间选择）
    pub fn generate_random_chrome_version() -> String {
        let mut rng = rand::rng();
        let versions = ["140", "142"];
        let idx = rng.random_range(0..versions.len());
        versions[idx].to_string()
    }

    /// 生成随机操作系统版本
    pub fn generate_random_system() -> &'static str {
        let systems = ["Windows 10", "Windows 11", "macOS"];
        let mut rng = rand::rng();
        let idx = rng.random_range(0..systems.len());
        systems[idx]
    }
}
