use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::msgpack::MsgPack;

/// 仕様 §5 のエラー。
///
/// `code` で分岐し `message` は表示にのみ使う、という契約を型で表す。
/// 文言を変えてもクライアントの分岐が壊れない。
#[derive(Debug)]
pub enum Error {
    /// リクエストの形式が不正。理由を添える。
    BadRequest(String),
    /// 上限を超えた (§6)。何の上限かを添える。
    LimitExceeded(String),
    /// 本文が 1 MiB を超えた (§6.1)。読まずに拒む。
    PayloadTooLarge,
    Unauthorized,
    InvalidSession,
    NotOwner,
    AlreadySynced,
    RoomNotFound,
    VersionMismatch,
    RoomFull,
    GameStarted,
    SyncTooOld,
    RoomLimitReached,
    Internal,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// 機械可読な識別子 (§5.2)。
    fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::LimitExceeded(_) | Self::PayloadTooLarge => "limit_exceeded",
            Self::Unauthorized => "unauthorized",
            Self::InvalidSession => "invalid_session",
            Self::NotOwner => "not_owner",
            Self::AlreadySynced => "already_synced",
            Self::RoomNotFound => "room_not_found",
            Self::VersionMismatch => "version_mismatch",
            Self::RoomFull => "room_full",
            Self::GameStarted => "game_started",
            Self::SyncTooOld => "sync_too_old",
            Self::RoomLimitReached => "room_limit_reached",
            Self::Internal => "internal",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) | Self::LimitExceeded(_) => StatusCode::BAD_REQUEST,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Unauthorized | Self::InvalidSession => StatusCode::UNAUTHORIZED,
            Self::NotOwner | Self::AlreadySynced => StatusCode::FORBIDDEN,
            Self::RoomNotFound => StatusCode::NOT_FOUND,
            Self::VersionMismatch | Self::RoomFull | Self::GameStarted => StatusCode::CONFLICT,
            Self::SyncTooOld => StatusCode::GONE,
            Self::RoomLimitReached => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// 人間向けの説明。UI に表示してよい。
    fn message(&self) -> String {
        match self {
            Self::BadRequest(reason) | Self::LimitExceeded(reason) => reason.clone(),
            Self::PayloadTooLarge => "送信されたデータが大きすぎます。".to_owned(),
            Self::Unauthorized => "パスワードが違います。".to_owned(),
            Self::InvalidSession => "接続が切れました。参加し直してください。".to_owned(),
            Self::NotOwner => "部屋主のみが実行できます。".to_owned(),
            Self::AlreadySynced => "同じ tick に二重に同期しようとしました。".to_owned(),
            Self::RoomNotFound => "部屋が見つかりません。番号を確認してください。".to_owned(),
            Self::VersionMismatch => {
                "バージョンが異なります。ゲームを更新してください。".to_owned()
            }
            Self::RoomFull => "部屋が満員です。".to_owned(),
            Self::GameStarted => "ゲームが既に始まっています。".to_owned(),
            Self::SyncTooOld => "同期が遅れすぎました。参加し直してください。".to_owned(),
            Self::RoomLimitReached => {
                "サーバーが混み合っています。しばらくしてからお試しください。".to_owned()
            }
            Self::Internal => "サーバー内部で問題が起きました。".to_owned(),
        }
    }
}

#[derive(Serialize)]
struct Body {
    error: Detail,
}

#[derive(Serialize)]
struct Detail {
    code: &'static str,
    message: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        if matches!(self, Self::Internal) {
            tracing::error!("内部エラーを返しました");
        }
        (
            self.status(),
            MsgPack(Body {
                error: Detail {
                    code: self.code(),
                    message: self.message(),
                },
            }),
        )
            .into_response()
    }
}
