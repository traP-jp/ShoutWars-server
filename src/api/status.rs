use axum::extract::State;
use serde::Serialize;

use crate::{AppState, msgpack::MsgPack};

/// `GET /v3/status` の応答 (仕様「GET /v3/status」)。
#[derive(Debug, Serialize)]
pub struct Status {
    room_count: usize,
    room_limit: usize,
}

pub async fn status(State(state): State<AppState>) -> MsgPack<Status> {
    MsgPack(Status {
        room_count: state.rooms.lock().count(),
        room_limit: state.config.room_limit,
    })
}
