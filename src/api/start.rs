//! `POST /v3/room/start`。

use axum::extract::State;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, error::Result, msgpack::MsgPack};

#[derive(Debug, Deserialize)]
pub struct Request {
    session_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct Response {}

pub async fn start(
    State(state): State<AppState>,
    MsgPack(request): MsgPack<Request>,
) -> Result<MsgPack<Response>> {
    state.rooms.lock().start(request.session_id)?;
    Ok(MsgPack(Response {}))
}
