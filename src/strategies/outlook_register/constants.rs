
pub const NEXT_BUTTON_SELECTORS: &[&str] = &[
    "#iSignupAction:visible",
    "input[type='submit']:visible",
    "button[type='submit']:visible",
    // English
    "button:has-text('Next'):visible",
    // Chinese Simplified
    "button:has-text('下一步'):visible",
    // Chinese Traditional
    "button:has-text('下一步'):visible",
    // Japanese
    "button:has-text('次へ'):visible",
    // Spanish
    "button:has-text('Siguiente'):visible",
    // French
    "button:has-text('Suivant'):visible",
    // German
    "button:has-text('Weiter'):visible",
    // Portuguese
    "button:has-text('Próximo'):visible",
    // Italian
    "button:has-text('Avanti'):visible",
    // Russian
    "button:has-text('Далее'):visible",
    // Korean
    "button:has-text('다음'):visible",
    // Vietnamese
    "button:has-text('Tiếp theo'):visible",
    // Thai
    "button:has-text('ถัดไป'):visible",
    // Indonesian
    "button:has-text('Berikutnya'):visible",
];

pub const AGREE_BUTTON_SELECTORS: &[&str] = &[
    // English
    "button:has-text('Agree and continue')",
    // Chinese Simplified
    "button:has-text('同意并继续')",
    "input[value='同意并继续']",
    // Chinese Traditional
    "button:has-text('同意並繼續')",
    // Japanese
    "button:has-text('同意して続行')",
    // Spanish
    "button:has-text('Aceptar y continuar')",
    // French
    "button:has-text('Accepter et continuer')",
    // German
    "button:has-text('Zustimmen und weiter')",
    // Portuguese
    "button:has-text('Aceitar e continuar')",
    // Italian
    "button:has-text('Accetta e continua')",
    // Russian
    "button:has-text('Принять и продолжить')",
    // Korean
    "button:has-text('동의 및 계속')",
];

pub const BIRTH_YEAR_SELECTORS: &[&str] = &[
    "input[name='BirthYear']",
    "input[id='BirthYear']",
    // English
    "[aria-label='Birth year']",
    // Chinese Simplified
    "[aria-label='出生年份']",
    // Chinese Traditional
    "[aria-label='出生年份']",
    // Japanese
    "[aria-label='誕生年']",
    // Spanish
    "[aria-label='Año de nacimiento']",
    // French
    "[aria-label='Année de naissance']",
    // German
    "[aria-label='Geburtsjahr']",
    // Portuguese
    "[aria-label='Ano de nascimento']",
    // Russian
    "[aria-label='Год рождения']",
    // Korean
    "[aria-label='출생 연도']",
];

pub const BIRTH_MONTH_SELECTORS: &[&str] = &[
    "select[name='BirthMonth']",
    "#BirthMonthDropdown",
    "[id='BirthMonthDropdown']",
    "[data-testid='BirthMonthDropdown']",
    "[data-testid='birth-month-dropdown']",
    // English
    "[aria-label='Birth month']",
    "[aria-label='Month']",
    // Chinese Simplified
    "[aria-label='出生月份']",
    "[aria-label='月']",
    // Chinese Traditional
    "[aria-label='出生月份']",
    // Japanese
    "[aria-label='誕生月']",
    "[aria-label='月']",
    // Spanish
    "[aria-label='Mes de nacimiento']",
    "[aria-label='Mes']",
    // French
    "[aria-label='Mois de naissance']",
    "[aria-label='Mois']",
    // German
    "[aria-label='Geburtsmonat']",
    "[aria-label='Monat']",
    // Portuguese
    "[aria-label='Mês de nascimento']",
    "[aria-label='Mês']",
    // Russian
    "[aria-label='Месяц рождения']",
    // Korean
    "[aria-label='출생 월']",
];

pub const BIRTH_DAY_SELECTORS: &[&str] = &[
    "#BirthDayDropdown",
    "[id='BirthDayDropdown']",
    // English
    "[aria-label='Birth day']",
    "[aria-label='Day']",
    // Chinese Simplified
    "[aria-label='出生日期']",
    "[aria-label='日']",
    // Chinese Traditional
    "[aria-label='出生日期']",
    // Japanese
    "[aria-label='誕生口']", // Note: Original code had this, possibly typo? keeping for compatibility
    "[aria-label='誕生日']",
    "[aria-label='日']",
    // Spanish
    "[aria-label='Día de nacimiento']",
    "[aria-label='Día']",
    // French
    "[aria-label='Jour de naissance']",
    "[aria-label='Jour']",
    // German
    "[aria-label='Geburtstag']",
    "[aria-label='Tag']",
    // Portuguese
    "[aria-label='Dia de nascimento']",
    "[aria-label='Dia']",
    // Russian
    "[aria-label='День рождения']",
    // Korean
    "[aria-label='출생 일']",
];

pub const FIRST_NAME_SELECTORS: &[&str] = &[
    "input[name='FirstName']",
    "input[id='FirstName']",
    "input[id='firstNameInput']",
    // English
    "[aria-label='First name']",
    // Chinese Simplified
    "[aria-label='名字']",
    "[aria-label='名']",
    // Japanese
    "[aria-label='名']",
    // Spanish
    "[aria-label='Nombre']",
    // French
    "[aria-label='Prénom']",
    // German
    "[aria-label='Vorname']",
    // Portuguese
    "[aria-label='Nome']",
    // Russian
    "[aria-label='Имя']",
    // Korean
    "[aria-label='이름']",
];

pub const LAST_NAME_SELECTORS: &[&str] = &[
    "input[name='LastName']",
    "input[id='LastName']",
    "input[id='lastNameInput']",
    // English
    "[aria-label='Last name']",
    // Chinese Simplified
    "[aria-label='姓氏']",
    "[aria-label='姓']",
    // Japanese
    "[aria-label='姓']",
    // Spanish
    "[aria-label='Apellidos']",
    // French
    "[aria-label='Nom']",
    // German
    "[aria-label='Nachname']",
    // Portuguese
    "[aria-label='Sobrenome']",
    // Russian
    "[aria-label='Фамилия']",
    // Korean
    "[aria-label='성']",
];

pub const BOT_KEYWORDS: &[&str] = &[
    "human", "robot", "puzzle", "verification", "challenge",
    "机器人", "验证", "证明", "人机",
    "人間", "証明", "ロボット",
    "humano", "robot", "verificación", "desafío",
    "humain", "vérification", "défi",
    "mensch", "verifizierung", "herausforderung",
    "humano", "verificação", "desafio",
    "человек", "робот", "проверка", "задача",
];

pub fn get_month_names(month: u32) -> Vec<&'static str> {
    match month {
        1 => vec!["January", "Janvier", "Enero", "Januar", "Gennaio", "Janeiro", "一月", "1月", "睦月", "Jan"],
        2 => vec!["February", "Février", "Febrero", "Februar", "Febbraio", "Fevereiro", "二月", "2月", "如月", "Feb"],
        3 => vec!["March", "Mars", "Marzo", "März", "Marzo", "Março", "三月", "3月", "弥生", "Mar"],
        4 => vec!["April", "Avril", "Abril", "April", "Aprile", "Abril", "四月", "4月", "卯月", "Apr"],
        5 => vec!["May", "Mai", "Mayo", "Mai", "Maggio", "Maio", "五月", "5月", "皐月", "May"],
        6 => vec!["June", "Juin", "Junio", "Juni", "Giugno", "Junho", "六月", "6月", "水無月", "Jun"],
        7 => vec!["July", "Juillet", "Julio", "Juli", "Luglio", "Julho", "七月", "7月", "文月", "Jul"],
        8 => vec!["August", "Août", "Agosto", "August", "Agosto", "Agosto", "八月", "8月", "葉月", "Aug"],
        9 => vec!["September", "Septembre", "Septiembre", "September", "Settembre", "Setembro", "九月", "9月", "長月", "Sep"],
        10 => vec!["October", "Octobre", "Octubre", "Oktober", "Ottobre", "Outubro", "十月", "10月", "神無月", "Oct"],
        11 => vec!["November", "Novembre", "Noviembre", "November", "Novembre", "Novembro", "十一月", "11月", "霜月", "Nov"],
        12 => vec!["December", "Décembre", "Diciembre", "Dezember", "Dicembre", "Dezembro", "十二月", "12月", "師走", "Dec"],
        _ => vec![],
    }
}
