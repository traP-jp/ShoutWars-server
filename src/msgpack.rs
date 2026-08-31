use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::Error;

/// MessagePack で本文をやり取りする (仕様 §4)。
///
/// マップのキーは名前で符号化する。クライアントは名前で読むため、
/// フィールド順に依存しない形式でなければならない。
#[derive(Debug)]
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

impl<T, S> FromRequest<S> for MsgPack<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let body = Bytes::from_request(request, state)
            .await
            .map_err(|reject| {
                if reject.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    Error::PayloadTooLarge
                } else {
                    Error::BadRequest("本文を読み取れませんでした。".to_owned())
                }
            })?;
        rmp_serde::from_slice(&body)
            .map(Self)
            .map_err(|error| Error::BadRequest(format!("本文の形式が正しくありません ({error})。")))
    }
}
