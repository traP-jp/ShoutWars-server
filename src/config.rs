use std::{env, fmt, str::FromStr, time::Duration};

/// 環境変数から読む設定。
///
/// 不正な値は既定値へフォールバックせず、起動を中止する。
#[derive(Debug, Clone)]
#[expect(
    dead_code,
    reason = "部屋の管理を実装するまで使わない。使い始めれば この属性自体が警告になる"
)]
pub struct Config {
    pub port: u16,
    pub password: Option<String>,
    pub room_limit: usize,
    pub lobby_lifetime: Duration,
    pub game_lifetime: Duration,
}

#[derive(Debug)]
pub struct ConfigError {
    name: &'static str,
    reason: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "環境変数 {} が不正です: {}", self.name, self.reason)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// 環境変数を読んで検証する。
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            port: parse("PORT", 7468)?,
            password: env::var("PASSWORD").ok().filter(|s| !s.is_empty()),
            room_limit: positive("ROOM_LIMIT", 100)?,
            lobby_lifetime: Duration::from_secs(positive::<u64>("LOBBY_LIFETIME", 10)? * 60),
            game_lifetime: Duration::from_secs(positive::<u64>("GAME_LIFETIME", 20)? * 60),
        })
    }
}

fn parse<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    match env::var(name) {
        Err(_) => Ok(default),
        Ok(raw) => raw.parse().map_err(|err: T::Err| ConfigError {
            name,
            reason: format!("{raw:?} を解釈できません ({err})"),
        }),
    }
}

fn positive<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: FromStr + Default + PartialOrd,
    T::Err: fmt::Display,
{
    let value = parse(name, default)?;
    if value <= T::default() {
        return Err(ConfigError {
            name,
            reason: "1 以上である必要があります".to_owned(),
        });
    }
    Ok(value)
}
