use std::{env, fmt, str::FromStr, time::Duration};

/// サーバーの設定。
///
/// 環境変数から読むのは、配備によって変える理由が実在する値だけ ([`Self::from_env`])。
/// 不正な値は既定値へフォールバックせず、起動を中止する。
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub password: Option<String>,
    pub room_limit: usize,
    pub lobby_lifetime: Duration,
    pub game_lifetime: Duration,
    /// tick の幅。参加時の応答で `tick_ms` として通知する。
    pub tick: Duration,
    /// 部屋ごとに保持する同期レコードの数。これより古い `next_tick` は追いつけない。
    pub record_retention: usize,
    /// 部屋ごとに保持するイベントの合計バイト数。超えた分は古いレコードから捨てる。
    ///
    /// 件数の上限だけでは、掛け合わせた量に上限が無く、上り帯域に比例して
    /// メモリを取られる。この値により、消費量は部屋数との積で抑えられる。
    pub room_memory_limit: usize,
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

/// 既定値。環境変数で変えられるものは README の表と、
/// 固定のものは `docs/protocol.md` の記載と一致していなければならない。
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 7468,
            password: None,
            room_limit: 100,
            // ゲームの制限時間ではなく、終わらない部屋を回収するための安全枠。
            // クライアントは残り時間を自前で数え、この値を受け取らない。環境変数にしてはならない。
            lobby_lifetime: Duration::from_mins(10),
            game_lifetime: Duration::from_mins(20),
            // 音声の単語検出に約 0.5 秒かかるため、これより短くしても入力遅延はほとんど縮まらない。
            // 縮めた分だけリクエストの頻度が上がるだけになる。
            tick: Duration::from_millis(100),
            record_retention: 100,
            // まっとうな 4 人部屋が保持期間いっぱいに使う量の 4 倍以上を見込む。
            room_memory_limit: 4 * 1024 * 1024,
        }
    }
}

impl Config {
    /// tick の幅をミリ秒で返す。クライアントへは `tick_ms` として渡す。
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
            room_memory_limit: mib("ROOM_MEMORY_LIMIT", default.room_memory_limit)?,
            ..default
        })
    }
}

/// MiB 単位で指定される大きさをバイト数で読む。
fn mib(name: &'static str, default: usize) -> Result<usize, ConfigError> {
    Ok(positive(name, default / (1024 * 1024))? * 1024 * 1024)
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
