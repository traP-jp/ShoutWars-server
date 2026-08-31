use axum::{
    body::Bytes,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// MessagePack として本文を返すレスポンス。
///
/// マップのキーは名前で符号化する。クライアントは名前で読むため、
/// フィールド順に依存しない形式でなければならない。
pub struct MsgPack<T>(pub T);

impl<T: Serialize> IntoResponse for MsgPack<T> {
    fn into_response(self) -> Response {
        match rmp_serde::to_vec_named(&self.0) {
            Ok(body) => (
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/msgpack"),
                )],
                Bytes::from(body),
            )
                .into_response(),
            Err(error) => {
                tracing::error!(%error, "MessagePack への符号化に失敗しました");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
