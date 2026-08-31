//! `POST /v3/room/sync`。

mod common;

use std::time::Duration;

use common::{Reply, TestServer};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shoutwars_server::config::Config;
use uuid::Uuid;

/// テストは実時間を待つため、tick を仕様の 100 ms より大幅に短くする。
fn 設定() -> Config {
    Config {
        tick: Duration::from_millis(50),
        record_retention: 5,
        ..Config::default()
    }
}

#[derive(Debug, Serialize)]
struct Create {
    version: String,
    user: UserName,
    size: usize,
}

#[derive(Debug, Serialize)]
struct Join {
    version: String,
    name: String,
    user: UserName,
}

#[derive(Debug, Serialize)]
struct UserName {
    name: String,
}

#[derive(Debug, Serialize, Default)]
struct Sync {
    session_id: String,
    next_tick: u64,
    applied: u64,
    reports: Vec<OutEvent>,
    actions: Vec<OutEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_info: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Status {
    room_count: usize,
}

/// `applied` を欠いた本文。
#[derive(Debug, Serialize)]
struct 申告なし {
    session_id: String,
    next_tick: u64,
}

#[derive(Debug, Serialize)]
struct Start {
    session_id: String,
}

#[derive(Debug, Serialize)]
struct OutEvent {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct Member {
    session_id: String,
    user_id: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Synced {
    next_tick: u64,
    room_users: Vec<WireUser>,
    started: bool,
    reports: Vec<InEvent>,
    actions: Vec<InEvent>,
    desync: bool,
}

#[derive(Debug, Deserialize)]
struct WireUser {
    id: String,
    name: String,
    absent: bool,
}

#[derive(Debug, Deserialize)]
struct InEvent {
    id: String,
    #[serde(default)]
    tick: Option<u64>,
    from: String,
    #[serde(rename = "type")]
    kind: String,
    data: String,
}

fn イベント(kind: &str, data: &str) -> OutEvent {
    OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: kind.to_owned(),
        data: data.to_owned(),
    }
}

async fn 部屋を作る(server: &TestServer, size: usize) -> Member {
    server
        .post(
            "/v3/room/create",
            &Create {
                version: "1.0".to_owned(),
                user: UserName {
                    name: "Alice".to_owned(),
                },
                size,
            },
        )
        .send()
        .await
        .msgpack()
}

async fn 参加する(server: &TestServer, number: &str, name: &str) -> Member {
    server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                name: number.to_owned(),
                user: UserName {
                    name: name.to_owned(),
                },
            },
        )
        .send()
        .await
        .msgpack()
}

fn 同期(session_id: &str, next_tick: u64) -> Sync {
    Sync {
        session_id: session_id.to_owned(),
        next_tick,
        ..Sync::default()
    }
}

async fn 送る(server: &TestServer, body: &Sync) -> Reply {
    server.post("/v3/room/sync", body).send().await
}

#[tokio::test]
async fn 一人でも同期できる() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let reply = 送る(&server, &同期(&alice.session_id, 0)).await;

    assert_eq!(reply.status, StatusCode::OK);
    let synced: Synced = reply.msgpack();
    assert!(synced.next_tick >= 1, "必ず 1 件以上のレコードを返す");
    assert_eq!(synced.room_users.len(), 1);
    assert_eq!(synced.room_users[0].id, alice.user_id);
    assert_eq!(synced.room_users[0].name, "Alice");
    assert!(
        !synced.room_users[0].absent,
        "同期した本人が不在になっています"
    );
    assert!(!synced.started);
    assert!(!synced.desync);
}

#[tokio::test]
async fn 確認イベントは送信者にも返る() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.actions = vec![イベント("attack", "えい")];
    let synced: Synced = 送る(&server, &body).await.msgpack();

    assert_eq!(synced.actions.len(), 1, "送信者にも返る");
    assert_eq!(synced.actions[0].kind, "attack");
    assert_eq!(synced.actions[0].data, "えい");
    assert_eq!(
        synced.actions[0].from, alice.user_id,
        "送信者はサーバーが埋める"
    );
}

#[tokio::test]
async fn 報告イベントは送信者に返らない() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.reports = vec![イベント("position", "3,4")];
    let synced: Synced = 送る(&server, &body).await.msgpack();

    assert!(synced.reports.is_empty(), "送信者には返さない");
}

#[tokio::test]
async fn 相手の報告は届く() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    let bob = 参加する(&server, &alice.name(), "Bob").await;

    let mut body = 同期(&bob.session_id, 0);
    body.reports = vec![イベント("position", "3,4")];
    let bob_reply = tokio::spawn({
        let server = server.clone();
        async move { 送る(&server, &body).await }
    });
    // Bob の預け入れが先に届くようにする。同じレコードに入れば全員到着で締め切られる。
    tokio::time::sleep(Duration::from_millis(10)).await;
    let alice_synced: Synced = 送る(&server, &同期(&alice.session_id, 0)).await.msgpack();
    bob_reply.await.expect("Bob の同期が終わりません");

    assert_eq!(alice_synced.reports.len(), 1);
    assert_eq!(alice_synced.reports[0].from, bob.user_id);
    assert_eq!(alice_synced.reports[0].data, "3,4");
}

#[tokio::test]
async fn 二重の同期は拒む() {
    // 窓を長く取り、1 本目が確実に届いてから 2 本目を送る。
    let Some(server) = TestServer::with_config(Config {
        tick: Duration::from_millis(500),
        ..設定()
    })
    .await
    else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    参加する(&server, &alice.name(), "Bob").await;

    // 相手が来ないので締め切りまで待つ。その間にもう一度送る。
    let first = tokio::spawn({
        let server = server.clone();
        let body = 同期(&alice.session_id, 0);
        async move { 送る(&server, &body).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let second = 送る(&server, &同期(&alice.session_id, 0)).await;
    first.await.expect("最初の同期が終わりません");

    assert_eq!(second.status, StatusCode::FORBIDDEN);
    assert_eq!(second.error_code(), "already_synced");
}

#[tokio::test]
async fn 古すぎるカーソルは拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    // 同期は続けるがカーソルを進めないクライアント。応答はあるので脱落はしないが、
    // 保持しているレコードから振り切られる。一人の部屋なので 1 回の同期で 1 tick 進む。
    let mut last = None;
    for _ in 0..8 {
        let reply = 送る(&server, &同期(&alice.session_id, 0)).await;
        if reply.status != StatusCode::OK {
            last = Some(reply);
            break;
        }
    }

    let reply = last.expect("保持期間を超えても拒まれませんでした");
    assert_eq!(reply.status, StatusCode::GONE);
    assert_eq!(reply.error_code(), "sync_too_old");
}

#[tokio::test]
async fn 未来のカーソルは拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let reply = 送る(&server, &同期(&alice.session_id, 9999)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn 無効なセッションは拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    部屋を作る(&server, 2).await;

    let reply = 送る(&server, &同期(&Uuid::new_v4().to_string(), 0)).await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn 件数の上限を超えたら拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.actions = (0..65).map(|_| イベント("spam", "x")).collect();
    let reply = 送る(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn 開始は同期の応答に現れる() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    server
        .post(
            "/v3/room/start",
            &Start {
                session_id: alice.session_id.clone(),
            },
        )
        .send()
        .await;
    let synced: Synced = 送る(&server, &同期(&alice.session_id, 0)).await.msgpack();

    assert!(synced.started, "開始が伝わっていません");
}

#[tokio::test]
async fn 複数レコードがまとめて返る() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(170)).await;

    let synced: Synced = 送る(&server, &同期(&alice.session_id, 0)).await.msgpack();

    assert!(
        synced.next_tick >= 3,
        "3 窓ぶん以上が締め切られているはず: {}",
        synced.next_tick
    );
}

async fn 部屋情報を送る(server: &TestServer, session_id: &str, info: &str) {
    let mut body = 同期(session_id, 0);
    body.room_info = Some(info.to_owned());
    let server = server.clone();
    tokio::spawn(async move { 送る(&server, &body).await });
    // 締め切りを跨がせる。反映はレコードの締め切り時である。
    tokio::time::sleep(Duration::from_millis(120)).await;
}

async fn 部屋情報を見る(server: &TestServer, number: &str, name: &str) -> Option<String> {
    let joined: RoomInfoOnly = server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                name: number.to_owned(),
                user: UserName {
                    name: name.to_owned(),
                },
            },
        )
        .send()
        .await
        .msgpack();
    joined.room_info
}

#[tokio::test]
async fn 部屋主は部屋情報を更新できる() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 4).await;

    部屋情報を送る(&server, &alice.session_id, "ステージ 2").await;

    assert_eq!(
        部屋情報を見る(&server, &alice.name(), "Bob")
            .await
            .as_deref(),
        Some("ステージ 2")
    );
}

#[tokio::test]
async fn 部屋主以外の部屋情報は無視する() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 4).await;
    let bob = 参加する(&server, &alice.name(), "Bob").await;

    部屋情報を送る(&server, &bob.session_id, "Bob の設定").await;

    assert_eq!(
        部屋情報を見る(&server, &alice.name(), "Charlie").await,
        None,
        "部屋主以外の更新が通りました"
    );
}

#[derive(Debug, Deserialize)]
struct RoomInfoOnly {
    room_info: Option<String>,
}

impl Member {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[tokio::test]
async fn 過去のレコードのイベントには番号が付く() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    // 一人だけの部屋では、預けた時点で全員到着となり即座に締め切られる。
    let mut first = 同期(&alice.session_id, 0);
    let 先のid = Uuid::now_v7().to_string();
    first.actions = vec![OutEvent {
        id: 先のid.clone(),
        kind: "attack".to_owned(),
        data: "A".to_owned(),
    }];
    送る(&server, &first).await;

    let mut second = 同期(&alice.session_id, 0);
    second.actions = vec![イベント("attack", "B")];
    let synced: Synced = 送る(&server, &second).await.msgpack();

    assert_eq!(synced.actions.len(), 2, "2 レコードぶんがまとまって返る");
    assert_eq!(synced.actions[0].data, "A");
    assert_eq!(synced.actions[0].id, 先のid, "イベント ID はそのまま返る");
    assert_eq!(
        synced.actions[0].tick,
        Some(0),
        "最後以外のレコードには番号が付く"
    );
    assert_eq!(synced.actions[1].data, "B");
    assert_eq!(
        synced.actions[1].tick, None,
        "最後のレコードのイベントには番号を付けない"
    );
}

fn 申告(session_id: &str, next_tick: u64, applied: u64) -> Sync {
    Sync {
        session_id: session_id.to_owned(),
        next_tick,
        applied,
        ..Sync::default()
    }
}

#[tokio::test]
async fn 申告が合っていれば異常としない() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut first = 申告(&alice.session_id, 0, 0);
    first.actions = vec![イベント("attack", "A")];
    let first: Synced = 送る(&server, &first).await.msgpack();
    assert_eq!(first.actions.len(), 1);
    assert!(!first.desync);

    // 1 件受け取ったので、次は applied = 1 を申告する。
    let second: Synced = 送る(&server, &申告(&alice.session_id, first.next_tick, 1))
        .await
        .msgpack();

    assert!(!second.desync, "正しい申告で desync と判定されました");
}

#[tokio::test]
async fn 申告がずれていれば検出する() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut first = 申告(&alice.session_id, 0, 0);
    first.actions = vec![イベント("attack", "A")];
    let first: Synced = 送る(&server, &first).await.msgpack();

    // 1 件配られたのに 99 件処理したと申告する。
    let second: Synced = 送る(&server, &申告(&alice.session_id, first.next_tick, 99))
        .await
        .msgpack();

    assert!(second.desync, "食い違いを検出できていません");
}

#[tokio::test]
async fn 検出したら全員に伝える() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    let bob = 参加する(&server, &alice.name(), "Bob").await;

    let bob_reply = tokio::spawn({
        let server = server.clone();
        let body = 申告(&bob.session_id, 0, 42);
        async move { 送る(&server, &body).await }
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let alice_synced: Synced = 送る(&server, &申告(&alice.session_id, 0, 0))
        .await
        .msgpack();
    bob_reply.await.expect("Bob の同期が終わりません");

    assert!(
        alice_synced.desync,
        "自分は正しくても、部屋の食い違いは伝わるべき"
    );
}

#[tokio::test]
async fn 申告が無い本文は拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let reply = server
        .post(
            "/v3/room/sync",
            &申告なし {
                session_id: alice.session_id.clone(),
                next_tick: 0,
            },
        )
        .send()
        .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn 応答が途絶えたユーザーは外れる() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    let bob = 参加する(&server, &alice.name(), "Bob").await;

    // Bob は一度も同期しない。保持数を超えて応答が無ければ部屋から外れる。
    let mut users = Vec::new();
    let mut cursor = 0;
    for _ in 0..8 {
        let synced: Synced = 送る(&server, &同期(&alice.session_id, cursor))
            .await
            .msgpack();
        cursor = synced.next_tick;
        users = synced.room_users;
        if users.len() == 1 {
            break;
        }
    }

    assert_eq!(users.len(), 1, "応答の無いユーザーが残っています");
    assert_eq!(users[0].name, "Alice");

    // 外れたユーザーのセッションは無効になる。
    let reply = 送る(&server, &同期(&bob.session_id, 0)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn 間に合わなかったユーザーは不在になる() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;
    参加する(&server, &alice.name(), "Bob").await;

    // Bob が来ないので、最初のレコードは期限で締め切られる。
    let synced: Synced = 送る(&server, &同期(&alice.session_id, 0)).await.msgpack();

    let bob = synced
        .room_users
        .iter()
        .find(|user| user.name == "Bob")
        .expect("Bob がまだ部屋にいるはず");
    assert!(bob.absent, "応答しなかったユーザーが不在になっていません");
    let alice_user = synced
        .room_users
        .iter()
        .find(|user| user.name == "Alice")
        .expect("Alice がいるはず");
    assert!(!alice_user.absent);
}

#[tokio::test]
async fn 大きすぎるデータは拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.actions = vec![OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: "attack".to_owned(),
        data: "あ".repeat(8 * 1024),
    }];
    let reply = 送る(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn 上限内のデータは通る() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.actions = vec![OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: "attack".to_owned(),
        // 文字列の符号化には長さの分も乗るため、上限より少し小さく取る。
        data: "a".repeat(8 * 1024 - 8),
    }];
    let reply = 送る(&server, &body).await;

    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn 大きすぎる部屋情報は拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.room_info = Some("あ".repeat(64 * 1024));
    let reply = 送る(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn 本文が大きすぎれば読まずに拒む() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    let mut body = 同期(&alice.session_id, 0);
    body.actions = (0..64)
        .map(|_| OutEvent {
            id: Uuid::now_v7().to_string(),
            kind: "attack".to_owned(),
            data: "a".repeat(20 * 1024),
        })
        .collect();
    let reply = 送る(&server, &body).await;

    // 1 MiB を超えるため、本文を復号する前に落ちる。
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn 全員が脱落した部屋は消える() {
    let Some(server) = TestServer::with_config(設定()).await else {
        return;
    };
    let alice = 部屋を作る(&server, 2).await;

    // Alice が同期を止めれば、保持数を超えたところで部屋には誰もいなくなる。
    tokio::time::sleep(Duration::from_millis(400)).await;
    let reply = 送る(&server, &同期(&alice.session_id, 0)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    let status: Status = server.get("/v3/status").send().await.msgpack();
    assert_eq!(status.room_count, 0, "誰もいない部屋が残っています");
}
