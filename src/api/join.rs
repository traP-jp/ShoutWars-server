//! `POST /v3/room/join`。

use axum::extract::State;
use rmpv::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, error::Result, msgpack::MsgPack, room::RoomNumber};

#[derive(Debug, Deserialize)]
pub struct Request {
    version: String,
    code: RoomNumber,
    user: UserName,
}

#[derive(Debug, Deserialize)]
struct UserName {
    name: String,
}

#[derive(Debug, Serialize)]
pub struct Response {
    session_id: Uuid,
    user_id: Uuid,
    id: Uuid,
    room_info: Value,
    /// 最初の同期で申告するカーソル。
    next_tick: u64,
    tick_ms: u64,
}

pub async fn join(
    State(state): State<AppState>,
    MsgPack(request): MsgPack<Request>,
) -> Result<MsgPack<Response>> {
    let mut rooms = state.rooms.lock();
    let joined = rooms.join(request.code, &request.version, request.user.name)?;
    Ok(MsgPack(Response {
        session_id: joined.session_id,
        user_id: joined.user_id,
        id: joined.room_id,
        room_info: joined.room_info,
        next_tick: joined.next_tick,
        tick_ms: state.config.tick_ms(),
    }))
}
