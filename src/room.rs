//! 部屋とユーザー (仕様「部屋のライフサイクル」)。

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    str::FromStr,
    time::{Duration, Instant},
};

use rmpv::Value;
use serde::{Deserialize, Serialize, de};
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    config::Config,
    error::Error,
    record::{Event, Record, UserSnapshot, merge},
};

/// 部屋番号 (仕様「部屋番号」)。口頭で伝えられるよう、部屋 ID とは別に持つ 6 桁の数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomNumber(u32);

impl RoomNumber {
    const DIGITS: u32 = 6;
    const RANGE: u32 = 10_u32.pow(Self::DIGITS);

    /// 暗号論的乱数で採番する (仕様「部屋番号」)。
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

/// 部屋への 1 回の参加 (仕様「ユーザー」)。同一人物との対応は保証しない。
#[derive(Debug)]
pub struct User {
    /// UUIDv7。参加順に増えるため、昇順に並べると先頭が部屋主になる (仕様「部屋主」)。
    pub id: Uuid,
    pub name: String,
    pub session_id: Uuid,
    /// 最後にイベントを預けたレコードの tick 番号 (仕様「遅延・不在・脱落・復帰」)。
    pub last_seen: u64,
}

impl User {
    /// ユーザー名の上限 (仕様「上限の一覧」)。
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
            // セッション ID は暗号論的乱数で生成する。UUIDv7 を使ってはならない (仕様「セッション」)。
            session_id: Uuid::new_v4(),
            last_seen: 0,
        })
    }
}

#[derive(Debug)]
pub struct Room {
    pub id: Uuid,
    pub number: RoomNumber,
    pub version: String,
    /// 部屋の人数の上限 (仕様「ユーザー」)。
    pub size: usize,
    pub created_at: Instant,
    /// ゲームを開始した時刻。ロビーの間は `None` (仕様「部屋の状態」)。
    pub started_at: Option<Instant>,
    /// ID 昇順。先頭が部屋主 (仕様「部屋主」)。
    pub users: Vec<User>,
    /// 遅延参加者へ渡す初期状態 (仕様「room_info」)。サーバーは中身を解釈しない (仕様「非責務」)。
    pub info: Value,
    /// 部屋主が送ってきた次の `info`。レコードの締め切り時に反映する (仕様「room_info」)。
    info_update: Option<Value>,
    /// 現在イベントを受け付けているレコードの tick 番号。
    open_tick: u64,
    /// 開いているレコードへ送信者ごとに溜めたイベント。
    pending: HashMap<Uuid, Pending>,
    /// 締め切り済みのレコード。古いものから捨てる (仕様「配送」)。
    closed: VecDeque<Record>,
    /// 直前のレコードに間に合わなかったユーザー (仕様「遅延・不在・脱落・復帰」)。バリアの待機対象から外す。
    absent: HashSet<Uuid>,
    /// 保持期間から落ちたレコードまでの累計配信数 (仕様「desync 検出」)。
    delivered_before: HashMap<Uuid, u64>,
    /// 食い違いを検出したか (仕様「desync 検出」)。一度立てば全員に通知し続ける。
    desync: bool,
    /// 締め切りを待っているリクエストを起こす。値は最後に締め切った tick。
    closed_notify: watch::Sender<Option<u64>>,
}

/// 開いているレコードへ、ある送信者が溜めたイベント。
#[derive(Debug, Default)]
struct Pending {
    reports: Vec<Event>,
    actions: Vec<Event>,
}

impl Room {
    /// 部屋の人数として許される範囲 (仕様「ユーザー」)。
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
            info: Value::Nil,
            info_update: None,
            open_tick: 0,
            pending: HashMap::new(),
            closed: VecDeque::new(),
            absent: HashSet::new(),
            delivered_before: HashMap::new(),
            desync: false,
            closed_notify: watch::Sender::new(None),
        })
    }

    /// 部屋そのものの期限 (仕様「部屋の状態」)。ロビーは作成から、ゲームは開始から数える。
    fn lifetime_deadline(&self, config: &Config) -> Instant {
        match self.started_at {
            Some(started_at) => started_at + config.game_lifetime,
            None => self.created_at + config.lobby_lifetime,
        }
    }

    pub fn is_expired(&self, config: &Config, now: Instant) -> bool {
        now >= self.lifetime_deadline(config)
    }

    /// 現在イベントを受け付けているレコードの tick 番号。
    pub fn open_tick(&self) -> u64 {
        self.open_tick
    }

    /// 開いているレコードの期限 (仕様「tick の進行」)。絶対時刻で持つため、締め切りが早まっても後ろへずれない。
    pub fn record_deadline(&self, tick: Duration) -> Instant {
        self.created_at + tick * u32::try_from(self.open_tick + 1).unwrap_or(u32::MAX)
    }

    pub fn subscribe(&self) -> watch::Receiver<Option<u64>> {
        self.closed_notify.subscribe()
    }

    /// 期限の過ぎたレコードを締め切り、応答の途絶えたユーザーを外す (仕様「tick の進行」、仕様「遅延・不在・脱落・復帰」)。
    ///
    /// 返すのは無効になったセッション。
    pub fn advance(&mut self, tick: Duration, now: Instant, retention: usize) -> Vec<Uuid> {
        while now >= self.record_deadline(tick) {
            self.close();
        }
        self.drop_silent(retention)
    }

    /// 待機対象の全員が到着したか (仕様「tick の進行」)。不在のユーザーは待たない (仕様「遅延・不在・脱落・復帰」)。
    pub fn everyone_arrived(&self) -> bool {
        !self.pending.is_empty()
            && self
                .users
                .iter()
                .filter(|user| !self.absent.contains(&user.id))
                .all(|user| self.pending.contains_key(&user.id))
    }

    /// 開いているレコードを締め切り、同時に次を開く (仕様「tick の進行」)。
    pub fn close(&mut self) {
        let tick = self.open_tick;
        let pending = std::mem::take(&mut self.pending);
        self.absent = self
            .users
            .iter()
            .map(|user| user.id)
            .filter(|id| !pending.contains_key(id))
            .collect();
        let users = self
            .users
            .iter()
            .map(|user| UserSnapshot {
                id: user.id,
                name: user.name.clone(),
                absent: self.absent.contains(&user.id),
            })
            .collect();

        let mut reports = Vec::new();
        let mut actions = Vec::new();
        for (sender, events) in pending {
            reports.push((sender, events.reports));
            actions.push((sender, events.actions));
        }
        // 部屋情報の差し替えも締め切りに合わせる。通知イベントと同じ境界で切り替わる (仕様「room_info」)。
        if let Some(info) = self.info_update.take() {
            self.info = info;
        }

        let mut record = Record {
            tick,
            reports: merge(tick, reports),
            actions: merge(tick, actions),
            users,
            started: self.started_at.is_some(),
            delivered: HashMap::new(),
        };
        record.delivered = self
            .users
            .iter()
            .map(|user| {
                let before = self.delivered_through(user.id);
                (user.id, before + record.delivered_to(user.id))
            })
            .collect();
        self.closed.push_back(record);
        self.open_tick += 1;
        let _ = self.closed_notify.send(Some(tick));
    }

    /// 最後に締め切ったレコードまでの累計配信数。
    fn delivered_through(&self, user: Uuid) -> u64 {
        self.closed
            .back()
            .map_or_else(
                || self.delivered_before.get(&user).copied(),
                |record| record.delivered.get(&user).copied(),
            )
            .unwrap_or(0)
    }

    /// `next_tick` の直前までに配った累計 (仕様「desync 検出」)。
    ///
    /// クライアントは `next_tick` より前をすべて処理し終えているはずなので、
    /// その件数が `applied` と一致する。
    fn delivered_before_tick(&self, user: Uuid, next_tick: u64) -> u64 {
        match self.closed.iter().find(|r| r.tick + 1 == next_tick) {
            Some(record) => record.delivered.get(&user).copied().unwrap_or(0),
            // 直前のレコードが保持期間から落ちている、または最初のレコードより前。
            None if self.oldest_tick() == Some(next_tick) => {
                self.delivered_before.get(&user).copied().unwrap_or(0)
            }
            None => 0,
        }
    }

    /// クライアントの申告と突き合わせる (仕様「desync 検出」)。一度でも食い違えば以後は立ったまま。
    ///
    /// ゲームの状態そのものをハッシュして突き合わせる方式は採らない。何を対象に含めるかを
    /// 全クライアントで揃え続ける必要があり、揃っていなければ誤検出する。件数の比較なら
    /// ゲームの内容に依存しない。増分ハッシュ (XOR) も適さない。自己逆元であるため、
    /// 検出したい二重適用がちょうど打ち消し合って一致してしまう。
    pub fn check_applied(&mut self, user: Uuid, next_tick: u64, applied: u64) {
        let expected = self.delivered_before_tick(user, next_tick);
        if applied != expected {
            tracing::warn!(%user, next_tick, applied, expected, "desync を検出しました");
            self.desync = true;
        }
    }

    pub fn is_desynced(&self) -> bool {
        self.desync
    }

    /// 保持期間を超えたレコードを捨てる (仕様「配送」)。
    pub fn trim(&mut self, retention: usize) {
        while self.closed.len() > retention {
            if let Some(dropped) = self.closed.pop_front() {
                self.delivered_before = dropped.delivered;
            }
        }
    }

    /// 保持している最も古いレコードの tick 番号。
    pub fn oldest_tick(&self) -> Option<u64> {
        self.closed.front().map(|record| record.tick)
    }

    /// `next_tick` 以降の締め切り済みレコード (仕様「配送」)。
    pub fn records_from(&self, next_tick: u64) -> impl Iterator<Item = &Record> {
        self.closed
            .iter()
            .filter(move |record| record.tick >= next_tick)
    }

    /// 開いているレコードへイベントを預ける。
    pub fn deposit(&mut self, from: Uuid, reports: Vec<Event>, actions: Vec<Event>) {
        let pending = self.pending.entry(from).or_default();
        pending.reports.extend(reports);
        pending.actions.extend(actions);
        if let Some(user) = self.users.iter_mut().find(|user| user.id == from) {
            user.last_seen = self.open_tick;
        }
    }

    /// 応答が途絶えたユーザーを部屋から除く (仕様「遅延・不在・脱落・復帰」)。
    ///
    /// 返すのは無効になったセッション。呼び出し側が登録簿から消す。
    /// 期間はレコードの保持数と揃える。仕様の 10 秒はどちらも同じ値であり、
    /// 一方だけを短くすると、部屋には残っているのに追いつけないユーザーが生じる。
    fn drop_silent(&mut self, retention: usize) -> Vec<Uuid> {
        let limit = u64::try_from(retention).unwrap_or(u64::MAX);
        let deadline = self.open_tick.saturating_sub(limit);
        let mut dropped = Vec::new();
        self.users.retain(|user| {
            if user.last_seen >= deadline {
                return true;
            }
            tracing::info!(user_id = %user.id, "応答が途絶えたため部屋から外しました");
            dropped.push(user.session_id);
            false
        });
        self.absent
            .retain(|id| self.users.iter().any(|user| user.id == *id));
        dropped
    }

    pub fn has_deposited(&self, user: Uuid) -> bool {
        self.pending.contains_key(&user)
    }

    /// 部屋主からの部屋情報を受け取る。反映は締め切り時 (仕様「room_info」)。
    pub fn queue_info(&mut self, info: Value) {
        self.info_update = Some(info);
    }

    /// 部屋主 (仕様「部屋主」)。ユーザーは ID 昇順に並ぶため、先頭が該当する。
    pub fn owner_id(&self) -> Option<Uuid> {
        self.users.first().map(|user| user.id)
    }

    pub fn is_full(&self) -> bool {
        self.users.len() >= self.size
    }
}
