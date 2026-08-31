//! `POST /v3/room/create` (仕様 §4.2)。

use axum::extract::State;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    error::Result,
    msgpack::MsgPack,
    room::{RoomNumber, User},
};

#[derive(Debug, Deserialize)]
pub struct Request {
    version: String,
    user: UserName,
    size: usize,
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
    name: RoomNumber,
    /// 最初の同期で申告する `last_tick`。作成直後なので 0 から始まる (§2.6)。
    tick: u64,
    tick_ms: u64,
}

pub async fn create(
    State(state): State<AppState>,
    MsgPack(request): MsgPack<Request>,
) -> Result<MsgPack<Response>> {
    let owner = User::new(request.user.name)?;
    let (session_id, user_id) = (owner.session_id, owner.id);

    let mut rooms = state.rooms.lock();
    let room = rooms.create(request.version, owner, request.size)?;
    Ok(MsgPack(Response {
        session_id,
        user_id,
        id: room.id,
        name: room.number,
        tick: 0,
        tick_ms: state.config.tick_ms(),
    }))
}
