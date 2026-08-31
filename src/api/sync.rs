//! `POST /v3/room/sync` (仕様 §4.4)。

use axum::extract::State;
use rmpv::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    error::{Error, Result},
    msgpack::MsgPack,
    record::{Event, Incoming, Record},
    room::Room,
    rooms::{Deposit, Sync, SyncRequest},
};

/// 1 リクエストに載せられるイベントの件数 (§6.1)。
///
/// 正常なクライアントは到達しない。壊れたクライアントに対する防波堤である。
const EVENT_LIMIT: usize = 64;

#[derive(Debug, Deserialize)]
pub struct Request {
    session_id: Uuid,
    last_tick: u64,
    #[serde(default)]
    reports: Vec<Incoming>,
    #[serde(default)]
    actions: Vec<Incoming>,
    /// 部屋主のみ有効 (§2.9)。
    #[serde(default)]
    room_info: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Response {
    /// 次回申告する `last_tick` (§2.6)。
    tick: u64,
    room_users: Vec<WireUser>,
    started: bool,
    reports: Vec<WireEvent>,
    actions: Vec<WireEvent>,
    desync: bool,
}

#[derive(Debug, Serialize)]
struct WireUser {
    id: Uuid,
    name: String,
    absent: bool,
}

#[derive(Debug, Serialize)]
struct WireEvent {
    id: Uuid,
    /// 最後のレコード以外に属する場合のみ入れる (§4.4)。
    #[serde(skip_serializing_if = "Option::is_none")]
    tick: Option<u64>,
    from: Uuid,
    #[serde(rename = "type")]
    kind: String,
    data: Value,
}

impl WireEvent {
    fn new(event: &Event, tick: Option<u64>) -> Self {
        Self {
            id: event.id,
            tick,
            from: event.from,
            kind: event.kind.clone(),
            data: event.data.clone(),
        }
    }
}

impl Response {
    /// `last_tick` 以降のレコードをまとめる。少なくとも 1 件あることが前提 (§2.11)。
    fn build(room: &Room, user: Uuid, last_tick: u64) -> Self {
        let records: Vec<&Record> = room.records_from(last_tick).collect();
        let last = records
            .last()
            .expect("返せるレコードがあると判定された後に呼ばれる");
        let (final_tick, users, started) = (last.tick, last.users.clone(), last.started);

        let mut reports = Vec::new();
        let mut actions = Vec::new();
        for record in records {
            let tick = (record.tick != final_tick).then_some(record.tick);
            // 報告イベントは送信者へ返さない (§2.2)。
            reports.extend(
                record
                    .reports
                    .iter()
                    .filter(|event| event.from != user)
                    .map(|event| WireEvent::new(event, tick)),
            );
            // 確認イベントは送信者にも返す (§2.3)。
            actions.extend(
                record
                    .actions
                    .iter()
                    .map(|event| WireEvent::new(event, tick)),
            );
        }

        Self {
            tick: final_tick + 1,
            room_users: users
                .into_iter()
                .map(|user| WireUser {
                    id: user.id,
                    name: user.name,
                    absent: user.absent,
                })
                .collect(),
            started,
            reports,
            actions,
            desync: false, // TODO: applied の照合 (§2.10)
        }
    }
}

pub async fn sync(
    State(state): State<AppState>,
    MsgPack(request): MsgPack<Request>,
) -> Result<MsgPack<Response>> {
    check_limit("reports", request.reports.len())?;
    check_limit("actions", request.actions.len())?;
    let (session_id, last_tick) = (request.session_id, request.last_tick);

    // イベントを預けるのは最初の 1 回だけ。待ち直しても二重に溜まらないようにする。
    // 送信者 ID はセッションを引いた後でなければ分からないため、ここでは埋めない。
    let mut deposit = Some(Deposit {
        reports: request.reports,
        actions: request.actions,
        room_info: request.room_info,
    });

    loop {
        let waiting = {
            let mut rooms = state.rooms.lock();
            match rooms.sync(SyncRequest {
                session_id,
                last_tick,
                deposit: deposit.take(),
            })? {
                Sync::Ready(user) => {
                    let room = rooms.room_of(session_id)?;
                    return Ok(MsgPack(Response::build(room, user, last_tick)));
                }
                Sync::Wait { deadline, closed } => (deadline, closed),
            }
        };
        let (deadline, mut closed) = waiting;
        tokio::select! {
            _ = closed.changed() => {}
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
        }
    }
}

fn check_limit(name: &str, count: usize) -> Result<()> {
    if count > EVENT_LIMIT {
        return Err(Error::LimitExceeded(format!(
            "{name} は 1 回につき {EVENT_LIMIT} 件までです。"
        )));
    }
    Ok(())
}
