use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::Error;

/// serde の human-readable 表現を選ぶ。
///
/// MessagePack 自体は人が読む形式ではないが、この切り替えは
/// 「コンパクトな表現」と「読める表現」のどちらを使うかを型に伝えるものであり、
/// UUID を 16 バイトの配列ではなく文字列として符号化させるために要る。
/// 仕様 §4 が `uuid` を「UUID の文字列表現」と定めているため、こちらを選ぶ。
fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let mut body = Vec::new();
    let mut serializer = rmp_serde::Serializer::new(&mut body)
        .with_struct_map()
        .with_human_readable();
    value.serialize(&mut serializer)?;
    Ok(body)
}

fn from_bytes<T: DeserializeOwned>(body: &[u8]) -> Result<T, rmp_serde::decode::Error> {
    let mut deserializer = rmp_serde::Deserializer::from_read_ref(&body).with_human_readable();
    T::deserialize(&mut deserializer)
}

/// MessagePack で本文をやり取りする (仕様 §4)。
///
/// マップのキーは名前で符号化する。クライアントは名前で読むため、
/// フィールド順に依存しない形式でなければならない。
#[derive(Debug)]
pub struct MsgPack<T>(pub T);

impl<T: Serialize> IntoResponse for MsgPack<T> {
    fn into_response(self) -> Response {
        match to_bytes(&self.0) {
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
        from_bytes(&body)
            .map(Self)
            .map_err(|error| Error::BadRequest(format!("本文の形式が正しくありません ({error})。")))
    }
}
