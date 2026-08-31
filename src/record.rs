//! 同期レコード (仕様 §2.1)。1 tick 分のイベント集合。

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use rmpv::Value;
use serde::Deserialize;
use uuid::Uuid;

/// イベント。`data` の中身はサーバーが解釈しない (§1.2)。
#[derive(Debug)]
pub struct Event {
    pub id: Uuid,
    pub from: Uuid,
    pub kind: String,
    pub data: Value,
}

/// クライアントから届いたイベント。
///
/// 送信者はサーバーが埋める。クライアントの申告を信じると、他人になりすませてしまう。
#[derive(Debug, Deserialize)]
pub struct Incoming {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub kind: String,
    pub data: Value,
}

impl Incoming {
    pub fn sent_by(self, from: Uuid) -> Event {
        Event {
            id: self.id,
            from,
            kind: self.kind,
            data: self.data,
        }
    }
}

/// レコード締め切り時点のユーザー (§2.8)。
#[derive(Debug, Clone)]
pub struct UserSnapshot {
    pub id: Uuid,
    pub name: String,
    pub absent: bool,
}

/// 締め切られたレコード。二度と変わらない (§2.11)。
#[derive(Debug)]
pub struct Record {
    pub tick: u64,
    pub reports: Vec<Event>,
    pub actions: Vec<Event>,
    pub users: Vec<UserSnapshot>,
    pub started: bool,
    /// このレコードまでにユーザーへ配った累計イベント数 (§2.10)。
    ///
    /// 参加より前のぶんは数えない。クライアントの `applied` と突き合わせる。
    pub delivered: HashMap<Uuid, u64>,
}

impl Record {
    /// このレコードで `user` へ配るイベントの件数。
    ///
    /// 報告イベントは送信者に返さないため (§2.2)、送信者ごとに数が違う。
    pub fn delivered_to(&self, user: Uuid) -> u64 {
        let own_reports = self
            .reports
            .iter()
            .filter(|event| event.from == user)
            .count();
        (self.reports.len() - own_reports + self.actions.len()) as u64
    }
}

/// この tick における送信者の順位 (§2.4)。
///
/// tick ごとに並びが入れ替わるため、特定のプレイヤーが恒久的に有利になることがない。
/// イベント ID の時刻部を使う方式と違い、クライアントのローカル時計に依存しない。
pub fn sender_rank(tick: u64, user: Uuid) -> u64 {
    let mut hasher = DefaultHasher::new();
    tick.hash(&mut hasher);
    user.hash(&mut hasher);
    hasher.finish()
}

/// 送信者ごとのイベントを、仕様の順序で 1 本に連結する (§2.4)。
///
/// 送信者の配列をそのまま繋ぐため、同一送信者内の順序は崩れない。
pub fn merge(tick: u64, mut per_sender: Vec<(Uuid, Vec<Event>)>) -> Vec<Event> {
    // 順位が衝突した場合に備え、ID で決定的に並べる。
    per_sender.sort_unstable_by_key(|(user, _)| (sender_rank(tick, *user), *user));
    per_sender
        .into_iter()
        .flat_map(|(_, events)| events)
        .collect()
}
