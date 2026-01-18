use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FacebookConfig {
    pub timeouts: Timeouts,
    pub selectors: Selectors,
    pub keywords: Keywords,
    pub urls: Urls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeouts {
    /// Login wait time in seconds
    pub login_wait_secs: u64,
    /// Page load wait time in seconds
    pub page_load_secs: u64,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            login_wait_secs: 8,
            page_load_secs: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selectors {
    pub login_form: LoginFormSelectors,
    pub indicators: SuccessIndicators,
    pub captcha: Vec<String>,
    pub error_containers: Vec<String>,
    pub two_fa_input: String,
    pub locked_indicators: Vec<String>,
    pub friends_count: Vec<String>,
}

impl Default for Selectors {
    fn default() -> Self {
        Self {
            login_form: LoginFormSelectors::default(),
            indicators: SuccessIndicators::default(),
            captcha: vec![
                "input[name='captcha_response']".to_string(),
                "iframe[src*='recaptcha']".to_string(),
                "iframe[title*='reCAPTCHA']".to_string(),
                "iframe[src*='hcaptcha']".to_string(),
                "div[data-testid='captcha']".to_string(),
                "div[id*='captcha']".to_string(),
                "img[alt*='captcha']".to_string(),
                "img[src*='captcha']".to_string(),
            ],
            error_containers: vec![
                "div[role='alert']".to_string(),
                "div._9ay7".to_string(),
                "#error_box".to_string(),
                "div[data-testid='error_message']".to_string(),
                "div[data-testid='royal_login_error']".to_string(),
            ],
            two_fa_input: "input[name='approvals_code']".to_string(),
            locked_indicators: vec![
                "div[data-testid='account_locked']".to_string(),
                "div[data-testid='checkpoint_locked']".to_string(),
                "button[name='submit[Continue]']".to_string(),
            ],
            friends_count: vec![
                "div[role='main'] h2".to_string(),
                "div[role='main'] span".to_string(),
                "a[href*='/friends/'] span".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFormSelectors {
    pub email: String,
    pub pass: String,
    pub login_btn: String,
}

impl Default for LoginFormSelectors {
    fn default() -> Self {
        Self {
            email: "input[name='email']".to_string(),
            pass: "input[name='pass']".to_string(),
            login_btn: "button[name='login']".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessIndicators {
    pub profile: Vec<String>,
    pub elements: Vec<String>,
}

impl Default for SuccessIndicators {
    fn default() -> Self {
        Self {
            profile: vec![
                "[aria-label*='Your profile']".to_string(),
                "[aria-label*='Account']".to_string(),
                "[aria-label*='个人主页']".to_string(),
            ],
            elements: vec![
                "[role='dialog']".to_string(),
                "div[role='main']".to_string(),
                "input[type='search']".to_string(),
                "input[aria-label*='Search']".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keywords {
    pub captcha: Vec<String>,
    pub wrong_password: Vec<String>,
    pub account_locked: Vec<String>,
    pub user_not_found: Vec<String>,
}

impl Default for Keywords {
    fn default() -> Self {
        Self {
            captcha: vec![
                "security check".to_string(),
                "security code".to_string(),
                "enter the code".to_string(),
                "captcha".to_string(),
                "验证码".to_string(),
                "安全检查".to_string(),
                "输入验证码".to_string(),
                "comprobación de seguridad".to_string(),
                "código de seguridad".to_string(),
                "ingresa el código".to_string(),
                "kiểm tra bảo mật".to_string(),
                "mã bảo mật".to_string(),
                "nhập mã".to_string(),
                "verificação de segurança".to_string(),
                "código de segurança".to_string(),
                "insira o código".to_string(),
                "vérification de sécurité".to_string(),
                "code de sécurité".to_string(),
                "entrez le code".to_string(),
                "sicherheitskontrolle".to_string(),
                "sicherheitscode".to_string(),
                "code eingeben".to_string(),
                "controllo di sicurezza".to_string(),
                "codice di sicurezza".to_string(),
                "inserisci il codice".to_string(),
                "güvenlik kontrolü".to_string(),
                "güvenlik kodu".to_string(),
                "kodu girin".to_string(),
                "pemeriksaan keamanan".to_string(),
                "kode keamanan".to_string(),
                "masukkan kode".to_string(),
                "การตรวจสอบความปลอดภัย".to_string(),
                "รหัสความปลอดภัย".to_string(),
                "ป้อนรหัส".to_string(),
                "fحص أمني".to_string(),
                "رمز الأمان".to_string(),
                "أدخل الرمز".to_string(),
            ],
            wrong_password: vec![
                "password you've entered is incorrect".to_string(),
                "wrong password".to_string(),
                "invalid password".to_string(),
                "密码错误".to_string(),
                "密码不正确".to_string(),
                "la contraseña es incorrecta".to_string(),
                "contraseña incorrecta".to_string(),
                "mật khẩu không đúng".to_string(),
                "mật khẩu sai".to_string(),
                "a senha está incorreta".to_string(),
                "senha incorreta".to_string(),
                "mot de passe incorrect".to_string(),
                "das passwort ist falsch".to_string(),
                "falsches passwort".to_string(),
                "password non corretta".to_string(),
                "şifre yanlış".to_string(),
                "kata sandi salah".to_string(),
                "รหัสผ่านไม่ถูกต้อง".to_string(),
                "كلمة السر غير صحيحة".to_string(),
                "パスワードが正しくありません".to_string(),
                "入力されたパスワードが間違っています".to_string(),
                "パスワードが間違っています".to_string(),
            ],
            account_locked: vec![
                "account locked".to_string(),
                "account disabled".to_string(),
                "temporarily locked".to_string(),
                "checkpoint".to_string(),
                "账号锁定".to_string(),
                "账号被封".to_string(),
                "暂时锁定".to_string(),
                "cuenta bloqueada".to_string(),
                "cuenta inhabilitada".to_string(),
                "bloqueada temporalmente".to_string(),
                "tài khoản bị khóa".to_string(),
                "tài khoản bị vô hiệu hóa".to_string(),
                "khóa tạm thời".to_string(),
                "conta bloqueada".to_string(),
                "conta desativada".to_string(),
                "bloqueada temporariamente".to_string(),
                "compte verrouillé".to_string(),
                "compte désactivé".to_string(),
                "verrouillé temporairement".to_string(),
                "konto gesperrt".to_string(),
                "konto deaktiviert".to_string(),
                "vorübergehend gesperrt".to_string(),
                "account bloccato".to_string(),
                "hesap kilitlendi".to_string(),
                "akun terkunci".to_string(),
                "บัญชีถูกล็อค".to_string(),
                "حساب مقفل".to_string(),
            ],
            user_not_found: vec![
                "email address you entered isn't connected to an account".to_string(),
                "isn't connected to an account".to_string(),
                "no account found".to_string(),
                "入力されたメールアドレスはアカウントにリンクされていません".to_string(),
                "アカウントにリンクされていません".to_string(),
                "没有找到账号".to_string(),
                "该邮箱未注册".to_string(),
                "el correo electrónico que ingresaste no está conectado a una cuenta".to_string(),
                "no está conectado a una cuenta".to_string(),
                "l'adresse e-mail que vous avez saisie n'est pas associée à un compte".to_string(),
                "n'est pas associée à un compte".to_string(),
                "e-mailadresse ist mit keinem konto verknüpft".to_string(),
                "mit keinem konto verknüpft".to_string(),
                "l'indirizzo e-mail inserito non è collegato a un account".to_string(),
                "non è collegato a un account".to_string(),
                "o email que você inseriu não está conectado a uma conta".to_string(),
                "não está conectado a uma conta".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Urls {
    pub base: String,
    pub friends: String,
    pub mobile_check: String,
    pub checkpoints: Vec<String>,
}

impl Default for Urls {
    fn default() -> Self {
        Self {
            base: "https://www.facebook.com".to_string(),
            friends: "https://www.facebook.com/me/friends".to_string(),
            mobile_check: "m.facebook.com".to_string(),
            checkpoints: vec!["828281030927956".to_string()],
        }
    }
}
