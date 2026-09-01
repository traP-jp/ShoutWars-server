//! `POST /v3/room/sync`。

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
    room_list::{Deposit, SyncOutcome, SyncRequest},
};

/// 1 リクエストに載せられるイベントの件数。
///
/// 正常なクライアントは到達しない。壊れたクライアントに対する防波堤である。
const EVENT_LIMIT: usize = 64;

/// 1 イベントの `data` のサイズ。
const DATA_LIMIT: usize = 8 * 1024;

/// `room_info` のサイズ。
const ROOM_INFO_LIMIT: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
pub struct Request {
    session_id: Uuid,
    next_tick: u64,
    /// 受け取って処理したイベントの累計。
    ///
    /// 既定値を持たせない。省略を 0 として扱うと、正常なクライアントがdesync と判定される。
    /// 届かなければ本文の不備として拒む。
    applied: u64,
    #[serde(default)]
    reports: Vec<Incoming>,
    #[serde(default)]
    actions: Vec<Incoming>,
    /// 部屋主のみ有効。
    #[serde(default)]
    room_info: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Response {
    /// 次のリクエストにそのまま入れる値。
    next_tick: u64,
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
    /// 最後のレコード以外に属する場合のみ入れる。
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
    /// `next_tick` 以降のレコードをまとめる。少なくとも 1 件あることが前提。
    fn build(room: &Room, user: Uuid, next_tick: u64) -> Self {
        let desync = room.is_desynced();
        let records: Vec<&Record> = room.records_from(next_tick).collect();
        let last = records
            .last()
            .expect("返せるレコードがあると判定された後に呼ばれる");
        let (final_tick, users, started) = (last.tick, last.users.clone(), last.started);

        let mut reports = Vec::new();
        let mut actions = Vec::new();
        for record in records {
            let tick = (record.tick != final_tick).then_some(record.tick);
            // 報告イベントは送信者へ返さない。
            reports.extend(
                record
                    .reports
                    .iter()
                    .filter(|event| event.from != user)
                    .map(|event| WireEvent::new(event, tick)),
            );
            // 確認イベントは送信者にも返す。
            actions.extend(
                record
                    .actions
                    .iter()
                    .map(|event| WireEvent::new(event, tick)),
            );
        }

        Self {
            next_tick: final_tick + 1,
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
            desync,
        }
    }
}

pub async fn sync(
    State(state): State<AppState>,
    MsgPack(request): MsgPack<Request>,
) -> Result<MsgPack<Response>> {
    check_count("reports", request.reports.len())?;
    check_count("actions", request.actions.len())?;
    for event in request.reports.iter().chain(&request.actions) {
        check_size("イベントの data", &event.data, DATA_LIMIT)?;
    }
    if let Some(info) = &request.room_info {
        check_size("room_info", info, ROOM_INFO_LIMIT)?;
    }
    let (session_id, next_tick) = (request.session_id, request.next_tick);

    // イベントを預けるのは最初の 1 回だけ。待ち直しても二重に溜まらないようにする。
    // 送信者 ID はセッションを引いた後でなければ分からないため、ここでは埋めない。
    let mut deposit = Some(Deposit {
        applied: request.applied,
        reports: request.reports,
        actions: request.actions,
        room_info: request.room_info,
    });

    loop {
        let waiting = {
            let mut rooms = state.rooms.lock();
            match rooms.sync(SyncRequest {
                session_id,
                next_tick,
                deposit: deposit.take(),
            })? {
                SyncOutcome::Ready(user) => {
                    let room = rooms.room_of(session_id)?;
                    return Ok(MsgPack(Response::build(room, user, next_tick)));
                }
                SyncOutcome::Wait { deadline, closed } => (deadline, closed),
            }
        };
        let (deadline, mut closed) = waiting;
        tokio::select! {
            _ = closed.changed() => {}
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
        }
    }
}

fn check_count(name: &str, count: usize) -> Result<()> {
    if count > EVENT_LIMIT {
        return Err(Error::LimitExceeded(format!(
            "{name} は 1 回につき {EVENT_LIMIT} 件までです。"
        )));
    }
    Ok(())
}

/// 符号化した長さで測る。中身は解釈しない。
fn check_size(name: &str, value: &Value, limit: usize) -> Result<()> {
    let size = rmp_serde::to_vec(value).map_err(|error| {
        tracing::error!(%error, "サイズを測れませんでした");
        Error::Internal
    })?;
    if size.len() > limit {
        return Err(Error::LimitExceeded(format!(
            "{name} は {} KiB までです。",
            limit / 1024
        )));
    }
    Ok(())
}
