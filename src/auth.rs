//! `PASSWORD` による認証。

use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

use crate::{AppState, error::Error};

/// `Authorization: Bearer <PASSWORD>` を検証する。
///
/// `PASSWORD` が未設定なら素通しする。
pub async fn require_password(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, Error> {
    let Some(expected) = state.config.password.as_deref() else {
        return Ok(next.run(request).await);
    };
    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| bearer(value.as_bytes()))
        .ok_or(Error::Unauthorized)?;

    // 一致・不一致で処理時間が変わらないようにする。長さの違いは隠せないが、
    // パスワードの内容を 1 文字ずつ探る攻撃を防ぐ。
    if bool::from(presented.ct_eq(expected.as_bytes())) {
        Ok(next.run(request).await)
    } else {
        Err(Error::Unauthorized)
    }
}

/// `Bearer <token>` からトークンを取り出す。スキーム名は大文字小文字を区別しない。
///
/// ヘッダ値を文字列として解釈しない。
/// 可視 ASCII 以外を含むパスワードでもそのまま比較できるようにするため、バイト列のまま扱う。
fn bearer(value: &[u8]) -> Option<&[u8]> {
    let index = value.iter().position(|byte| *byte == b' ')?;
    let (scheme, token) = value.split_at(index);
    scheme.eq_ignore_ascii_case(b"Bearer").then(|| &token[1..])
}
