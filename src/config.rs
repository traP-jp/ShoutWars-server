use std::{env, fmt, str::FromStr, time::Duration};

/// 環境変数から読む設定。
///
/// 不正な値は既定値へフォールバックせず、起動を中止する。
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub password: Option<String>,
    pub room_limit: usize,
    pub lobby_lifetime: Duration,
    pub game_lifetime: Duration,
    /// tick の幅 (仕様「tick の進行」)。参加時の応答で `tick_ms` として通知する。
    pub tick: Duration,
    /// 部屋ごとに保持する同期レコードの数 (仕様「配送」)。
    /// これより古い `next_tick` は追いつけない。
    pub record_retention: usize,
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

/// 環境変数が無い場合の値。README の表と一致していなければならない。
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 7468,
            password: None,
            room_limit: 100,
            lobby_lifetime: Duration::from_mins(10),
            game_lifetime: Duration::from_mins(20),
            // 音声の単語検出に約 0.5 秒かかるため、これより短くしても入力遅延はほとんど
            // 縮まらない。縮めた分だけリクエストの頻度が上がるだけになる。
            tick: Duration::from_millis(100),
            record_retention: 100,
        }
    }
}

impl Config {
    /// tick の幅をミリ秒で返す。クライアントへは `tick_ms` として渡す (仕様「tick の進行」)。
    #[must_use]
    pub fn tick_ms(&self) -> u64 {
        u64::try_from(self.tick.as_millis()).unwrap_or(u64::MAX)
    }

    /// 環境変数を読んで検証する。
    ///
    /// # Errors
    /// 値を解釈できない、または許される範囲を外れている場合。
    pub fn from_env() -> Result<Self, ConfigError> {
        let default = Self::default();
        Ok(Self {
            port: parse("PORT", default.port)?,
            password: env::var("PASSWORD").ok().filter(|s| !s.is_empty()),
            room_limit: positive("ROOM_LIMIT", default.room_limit)?,
            lobby_lifetime: minutes("LOBBY_LIFETIME", default.lobby_lifetime)?,
            game_lifetime: minutes("GAME_LIFETIME", default.game_lifetime)?,
            tick: millis("TICK_MS", default.tick)?,
            record_retention: positive("RECORD_RETENTION", default.record_retention)?,
        })
    }
}

/// ミリ秒単位で指定される時間を読む。
fn millis(name: &'static str, default: Duration) -> Result<Duration, ConfigError> {
    let ms: u64 = default
        .as_millis()
        .try_into()
        .expect("既定値がミリ秒として大きすぎます");
    Ok(Duration::from_millis(positive(name, ms)?))
}

/// 分単位で指定される時間を読む。
fn minutes(name: &'static str, default: Duration) -> Result<Duration, ConfigError> {
    Ok(Duration::from_secs(
        positive(name, default.as_secs() / 60)? * 60,
    ))
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
