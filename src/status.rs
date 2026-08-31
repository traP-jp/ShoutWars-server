use axum::extract::State;
use serde::Serialize;

use crate::{AppState, msgpack::MsgPack};

/// `GET /v3/status` の応答 (仕様 §4.6)。
#[derive(Debug, Serialize)]
pub(crate) struct Status {
    room_count: usize,
    room_limit: usize,
}

pub(crate) async fn status(State(state): State<AppState>) -> MsgPack<Status> {
    MsgPack(Status {
        room_count: 0, // TODO: 部屋の管理を実装したら差し替える
        room_limit: state.room_limit,
    })
}
