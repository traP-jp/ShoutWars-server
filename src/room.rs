//! 部屋とユーザー (仕様 §3)。

use std::{fmt, str::FromStr, time::Instant};

use serde::{Deserialize, Serialize, de};
use uuid::Uuid;

use crate::{config::Config, error::Error};

/// 部屋番号 (仕様 §3.2)。口頭で伝えられるよう、部屋 ID とは別に持つ 6 桁の数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomNumber(u32);

impl RoomNumber {
    const DIGITS: u32 = 6;
    const RANGE: u32 = 10_u32.pow(Self::DIGITS);

    /// 暗号論的乱数で採番する (§3.2)。
    ///
    /// 剰余による偏りは `2^32 / 10^6` の端数ぶんで、10 万分の 1 未満。
    /// 番号は総当たりを困難にするためのものであり、この偏りは問題にならない。
    pub fn random() -> Self {
        let mut bytes = [0_u8; 4];
        getrandom::fill(&mut bytes).expect("乱数を取得できません");
        Self(u32::from_le_bytes(bytes) % Self::RANGE)
    }
}

impl fmt::Display for RoomNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:0>width$}", self.0, width = Self::DIGITS as usize)
    }
}

/// 部屋番号として読めなかったことを表す。
///
/// `Error` と分けてあるのは、serde が要求する `Display` を満たすため。
/// `Error` は HTTP の応答を組み立てる型であり、文字列にする意味を持たない。
#[derive(Debug)]
pub struct InvalidRoomNumber;

impl fmt::Display for InvalidRoomNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "部屋番号は {} 桁の数字です。", RoomNumber::DIGITS)
    }
}

impl From<InvalidRoomNumber> for Error {
    fn from(error: InvalidRoomNumber) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl FromStr for RoomNumber {
    type Err = InvalidRoomNumber;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != Self::DIGITS as usize || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(InvalidRoomNumber);
        }
        text.parse().map(Self).map_err(|_| InvalidRoomNumber)
    }
}

/// 番号は数字の文字列として送る。先頭の 0 を落とさないため、整数にはしない。
impl Serialize for RoomNumber {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RoomNumber {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

/// 部屋への 1 回の参加 (仕様 §3.3)。同一人物との対応は保証しない。
#[derive(Debug)]
#[expect(dead_code, reason = "join の実装で読む")]
pub struct User {
    /// UUIDv7。参加順に増えるため、昇順に並べると先頭が部屋主になる (§3.4)。
    pub id: Uuid,
    pub name: String,
    pub session_id: Uuid,
}

impl User {
    /// ユーザー名の上限 (§6.1)。
    const NAME_LIMIT: usize = 32;

    /// # Errors
    /// ユーザー名が上限を超えている場合。
    pub fn new(name: String) -> Result<Self, Error> {
        if name.chars().count() > Self::NAME_LIMIT {
            return Err(Error::BadRequest(format!(
                "ユーザー名は {} 文字以内です。",
                Self::NAME_LIMIT
            )));
        }
        Ok(Self {
            id: Uuid::now_v7(),
            name,
            // セッション ID は暗号論的乱数で生成する。UUIDv7 を使ってはならない (§3.6)。
            session_id: Uuid::new_v4(),
        })
    }
}

#[derive(Debug)]
#[expect(dead_code, reason = "join の実装で読む")]
pub struct Room {
    pub id: Uuid,
    pub number: RoomNumber,
    pub version: String,
    /// 部屋の人数の上限 (§3.3)。
    pub size: usize,
    pub created_at: Instant,
    /// ゲームを開始した時刻。ロビーの間は `None` (§3.1)。
    pub started_at: Option<Instant>,
    /// ID 昇順。先頭が部屋主 (§3.4)。
    pub users: Vec<User>,
}

impl Room {
    /// 部屋の人数として許される範囲 (§3.3)。
    const SIZE: std::ops::RangeInclusive<usize> = 2..=4;

    /// # Errors
    /// 人数が範囲外の場合。
    pub fn new(
        number: RoomNumber,
        version: String,
        owner: User,
        size: usize,
    ) -> Result<Self, Error> {
        if !Self::SIZE.contains(&size) {
            return Err(Error::BadRequest(format!(
                "部屋の人数は {} 〜 {} 人です。",
                Self::SIZE.start(),
                Self::SIZE.end()
            )));
        }
        Ok(Self {
            id: Uuid::new_v4(),
            number,
            version,
            size,
            created_at: Instant::now(),
            started_at: None,
            users: vec![owner],
        })
    }

    /// 期限 (§3.1)。ロビーは作成から、ゲームは開始から数える。
    fn deadline(&self, config: &Config) -> Instant {
        match self.started_at {
            Some(started_at) => started_at + config.game_lifetime,
            None => self.created_at + config.lobby_lifetime,
        }
    }

    pub fn is_expired(&self, config: &Config, now: Instant) -> bool {
        now >= self.deadline(config)
    }
}
