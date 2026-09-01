//! `POST /v3/room/sync`。

mod common;

use std::time::Duration;

use common::{Reply, TestServer};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shoutwars_server::config::Config;
use uuid::Uuid;

/// テストは実時間を待つため、tick を仕様の 100 ms より大幅に短くする。
fn config() -> Config {
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
    code: String,
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
struct BodyWithoutApplied {
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
    code: String,
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

fn event(kind: &str, data: &str) -> OutEvent {
    OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: kind.to_owned(),
        data: data.to_owned(),
    }
}

async fn create_room(server: &TestServer, size: usize) -> Member {
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

async fn join_room(server: &TestServer, code: &str, name: &str) -> Member {
    server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                code: code.to_owned(),
                user: UserName {
                    name: name.to_owned(),
                },
            },
        )
        .send()
        .await
        .msgpack()
}

fn sync_request(session_id: &str, next_tick: u64) -> Sync {
    Sync {
        session_id: session_id.to_owned(),
        next_tick,
        ..Sync::default()
    }
}

async fn post_sync(server: &TestServer, body: &Sync) -> Reply {
    server.post("/v3/room/sync", body).send().await
}

#[tokio::test]
async fn syncs_with_a_single_user() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let reply = post_sync(&server, &sync_request(&alice.session_id, 0)).await;

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
async fn actions_are_echoed_to_the_sender() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = vec![event("attack", "えい")];
    let synced: Synced = post_sync(&server, &body).await.msgpack();

    assert_eq!(synced.actions.len(), 1, "送信者にも返る");
    assert_eq!(synced.actions[0].kind, "attack");
    assert_eq!(synced.actions[0].data, "えい");
    assert_eq!(
        synced.actions[0].from, alice.user_id,
        "送信者はサーバーが埋める"
    );
}

#[tokio::test]
async fn reports_are_not_echoed_to_the_sender() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.reports = vec![event("position", "3,4")];
    let synced: Synced = post_sync(&server, &body).await.msgpack();

    assert!(synced.reports.is_empty(), "送信者には返さない");
}

#[tokio::test]
async fn reports_reach_the_other_users() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;
    let bob = join_room(&server, &alice.code(), "Bob").await;

    let mut body = sync_request(&bob.session_id, 0);
    body.reports = vec![event("position", "3,4")];
    let bob_reply = tokio::spawn({
        let server = server.clone();
        async move { post_sync(&server, &body).await }
    });
    // Bob の預け入れが先に届くようにする。同じレコードに入れば全員到着で締め切られる。
    tokio::time::sleep(Duration::from_millis(10)).await;
    let alice_synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();
    bob_reply.await.expect("Bob の同期が終わりません");

    assert_eq!(alice_synced.reports.len(), 1);
    assert_eq!(alice_synced.reports[0].from, bob.user_id);
    assert_eq!(alice_synced.reports[0].data, "3,4");
}

#[tokio::test]
async fn rejects_a_double_sync() {
    // 窓を長く取り、1 本目が確実に届いてから 2 本目を送る。
    let Some(server) = TestServer::with_config(Config {
        tick: Duration::from_millis(500),
        ..config()
    })
    .await
    else {
        return;
    };
    let alice = create_room(&server, 2).await;
    join_room(&server, &alice.code(), "Bob").await;

    // 相手が来ないので締め切りまで待つ。その間にもう一度送る。
    let first = tokio::spawn({
        let server = server.clone();
        let body = sync_request(&alice.session_id, 0);
        async move { post_sync(&server, &body).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let second = post_sync(&server, &sync_request(&alice.session_id, 0)).await;
    first.await.expect("最初の同期が終わりません");

    assert_eq!(second.status, StatusCode::FORBIDDEN);
    assert_eq!(second.error_code(), "already_synced");
}

#[tokio::test]
async fn rejects_a_cursor_that_is_too_old() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    // 同期は続けるがカーソルを進めないクライアント。応答はあるので脱落はしないが、
    // 保持しているレコードから振り切られる。一人の部屋なので 1 回の同期で 1 tick 進む。
    let mut last = None;
    for _ in 0..8 {
        let reply = post_sync(&server, &sync_request(&alice.session_id, 0)).await;
        if reply.status != StatusCode::OK {
            last = Some(reply);
            break;
        }
        // 同じ窓へ二度送らないよう、tick 幅を空ける。
        tokio::time::sleep(config().tick).await;
    }

    let reply = last.expect("保持期間を超えても拒まれませんでした");
    assert_eq!(reply.status, StatusCode::GONE);
    assert_eq!(reply.error_code(), "sync_too_old");
}

#[tokio::test]
async fn rejects_a_cursor_in_the_future() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let reply = post_sync(&server, &sync_request(&alice.session_id, 9999)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "bad_request");
}

#[tokio::test]
async fn rejects_an_invalid_session() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    create_room(&server, 2).await;

    let reply = post_sync(&server, &sync_request(&Uuid::new_v4().to_string(), 0)).await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn rejects_too_many_events() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = (0..65).map(|_| event("spam", "x")).collect();
    let reply = post_sync(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn the_start_appears_in_the_response() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    server
        .post(
            "/v3/room/start",
            &Start {
                session_id: alice.session_id.clone(),
            },
        )
        .send()
        .await;
    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();

    assert!(synced.started, "開始が伝わっていません");
}

#[tokio::test]
async fn returns_several_records_at_once() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(170)).await;

    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();

    assert!(
        synced.next_tick >= 3,
        "3 窓ぶん以上が締め切られているはず: {}",
        synced.next_tick
    );
}

async fn send_room_info(server: &TestServer, session_id: &str, info: &str) {
    let mut body = sync_request(session_id, 0);
    body.room_info = Some(info.to_owned());
    let server = server.clone();
    tokio::spawn(async move { post_sync(&server, &body).await });
    // 締め切りを跨がせる。反映はレコードの締め切り時である。
    tokio::time::sleep(Duration::from_millis(120)).await;
}

async fn read_room_info(server: &TestServer, code: &str, name: &str) -> Option<String> {
    // エラー本文も `Option` の `None` として読めてしまうため、状態を先に確かめる。
    let joined: RoomInfoOnly = server
        .post(
            "/v3/room/join",
            &Join {
                version: "1.0".to_owned(),
                code: code.to_owned(),
                user: UserName {
                    name: name.to_owned(),
                },
            },
        )
        .send()
        .await
        .expect_ok();
    joined.room_info
}

#[tokio::test]
async fn the_owner_can_update_room_info() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 4).await;

    send_room_info(&server, &alice.session_id, "ステージ 2").await;

    assert_eq!(
        read_room_info(&server, &alice.code(), "Bob")
            .await
            .as_deref(),
        Some("ステージ 2")
    );
}

#[tokio::test]
async fn ignores_room_info_from_others() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 4).await;
    let bob = join_room(&server, &alice.code(), "Bob").await;

    send_room_info(&server, &bob.session_id, "Bob の設定").await;

    assert_eq!(
        read_room_info(&server, &alice.code(), "Charlie").await,
        None,
        "部屋主以外の更新が通りました"
    );
}

#[derive(Debug, Deserialize)]
struct RoomInfoOnly {
    room_info: Option<String>,
}

impl Member {
    fn code(&self) -> String {
        self.code.clone()
    }
}

#[tokio::test]
async fn the_last_record_carries_no_tick() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = vec![event("attack", "A")];
    let synced: Synced = post_sync(&server, &body).await.msgpack();

    assert_eq!(synced.actions.len(), 1);
    assert_eq!(
        synced.actions[0].tick, None,
        "最後のレコードの event には番号を付けない"
    );
}

#[tokio::test]
async fn older_records_carry_a_tick() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let first_id = Uuid::now_v7().to_string();
    let mut first = sync_request(&alice.session_id, 0);
    first.actions = vec![OutEvent {
        id: first_id.clone(),
        kind: "attack".to_owned(),
        data: "A".to_owned(),
    }];
    let first: Synced = post_sync(&server, &first).await.msgpack();

    let mut second = sync_request(&alice.session_id, first.next_tick);
    second.actions = vec![event("attack", "B")];
    post_sync(&server, &second).await;

    // 先頭から取り直すと、2 つの event は別のレコードに入って返る。
    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();

    let data: Vec<&str> = synced.actions.iter().map(|e| e.data.as_str()).collect();
    assert_eq!(data, ["A", "B"], "送った順に並ぶ");
    assert_eq!(synced.actions[0].id, first_id, "event ID はそのまま返る");
    let older = synced.actions[0]
        .tick
        .expect("最後以外のレコードには番号が付く");
    // 最後のレコードに入っていれば番号は付かない。
    if let Some(newer) = synced.actions[1].tick {
        assert!(newer > older, "後の event が古いレコードに入っています");
    }
}

fn sync_request_with_applied(session_id: &str, next_tick: u64, applied: u64) -> Sync {
    Sync {
        session_id: session_id.to_owned(),
        next_tick,
        applied,
        ..Sync::default()
    }
}

#[tokio::test]
async fn accepts_a_matching_applied_count() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut first = sync_request_with_applied(&alice.session_id, 0, 0);
    first.actions = vec![event("attack", "A")];
    let first: Synced = post_sync(&server, &first).await.msgpack();
    assert_eq!(first.actions.len(), 1);
    assert!(!first.desync);

    // 1 件受け取ったので、次は applied = 1 を申告する。
    let second: Synced = post_sync(
        &server,
        &sync_request_with_applied(&alice.session_id, first.next_tick, 1),
    )
    .await
    .msgpack();

    assert!(!second.desync, "正しい申告で desync と判定されました");
}

#[tokio::test]
async fn detects_a_mismatched_applied_count() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut first = sync_request_with_applied(&alice.session_id, 0, 0);
    first.actions = vec![event("attack", "A")];
    let first: Synced = post_sync(&server, &first).await.msgpack();

    // 1 件配られたのに 99 件処理したと申告する。
    let second: Synced = post_sync(
        &server,
        &sync_request_with_applied(&alice.session_id, first.next_tick, 99),
    )
    .await
    .msgpack();

    assert!(second.desync, "食い違いを検出できていません");
}

#[tokio::test]
async fn reports_desync_to_everyone() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;
    let bob = join_room(&server, &alice.code(), "Bob").await;

    let bob_reply = tokio::spawn({
        let server = server.clone();
        let body = sync_request_with_applied(&bob.session_id, 0, 42);
        async move { post_sync(&server, &body).await }
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let alice_synced: Synced =
        post_sync(&server, &sync_request_with_applied(&alice.session_id, 0, 0))
            .await
            .msgpack();
    bob_reply.await.expect("Bob の同期が終わりません");

    assert!(
        alice_synced.desync,
        "自分は正しくても、部屋の食い違いは伝わるべき"
    );
}

#[tokio::test]
async fn rejects_a_body_without_applied() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let reply = server
        .post(
            "/v3/room/sync",
            &BodyWithoutApplied {
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
async fn drops_users_that_stop_responding() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;
    let bob = join_room(&server, &alice.code(), "Bob").await;

    // Bob は一度も同期しない。保持数を超えて応答が無ければ部屋から外れる。
    let mut users = Vec::new();
    let mut cursor = 0;
    for _ in 0..8 {
        let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, cursor))
            .await
            .expect_ok();
        cursor = synced.next_tick;
        users = synced.room_users;
        if users.len() == 1 {
            break;
        }
    }

    assert_eq!(users.len(), 1, "応答の無いユーザーが残っています");
    assert_eq!(users[0].name, "Alice");

    // 外れたユーザーのセッションは無効になる。
    let reply = post_sync(&server, &sync_request(&bob.session_id, 0)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn marks_late_users_as_absent() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;
    join_room(&server, &alice.code(), "Bob").await;

    // Bob が来ないので、最初のレコードは期限で締め切られる。
    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();

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

/// MessagePack の文字列は、長さ 256〜65535 なら 3 バイトの見出しが付く。
/// 上限は符号化後の大きさで測るので、境界はその分だけ内側にある。
const DATA_LIMIT: usize = 8 * 1024 - 3;
const ROOM_INFO_LIMIT: usize = 64 * 1024 - 3;

#[tokio::test]
async fn accepts_data_at_the_limit() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = vec![OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: "attack".to_owned(),
        data: "a".repeat(DATA_LIMIT),
    }];

    assert_eq!(post_sync(&server, &body).await.status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_data_one_byte_over_the_limit() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = vec![OutEvent {
        id: Uuid::now_v7().to_string(),
        kind: "attack".to_owned(),
        data: "a".repeat(DATA_LIMIT + 1),
    }];
    let reply = post_sync(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn accepts_room_info_at_the_limit() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.room_info = Some("a".repeat(ROOM_INFO_LIMIT));

    assert_eq!(post_sync(&server, &body).await.status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_room_info_one_byte_over_the_limit() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.room_info = Some("a".repeat(ROOM_INFO_LIMIT + 1));
    let reply = post_sync(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn accepts_the_maximum_number_of_events() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = (0..64).map(|_| event("attack", "x")).collect();
    body.reports = (0..64).map(|_| event("position", "x")).collect();

    assert_eq!(post_sync(&server, &body).await.status, StatusCode::OK);
}

#[tokio::test]
async fn rejects_too_many_reports() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.reports = (0..65).map(|_| event("position", "x")).collect();
    let reply = post_sync(&server, &body).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn rejects_an_oversized_body() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = (0..64)
        .map(|_| OutEvent {
            id: Uuid::now_v7().to_string(),
            kind: "attack".to_owned(),
            data: "a".repeat(20 * 1024),
        })
        .collect();
    let reply = post_sync(&server, &body).await;

    // 1 MiB を超えるため、本文を復号する前に落ちる。
    assert_eq!(reply.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(reply.error_code(), "limit_exceeded");
}

#[tokio::test]
async fn removes_a_room_once_everyone_is_gone() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    // Alice が同期を止めれば、保持数を超えたところで部屋には誰もいなくなる。
    tokio::time::sleep(Duration::from_millis(400)).await;
    let reply = post_sync(&server, &sync_request(&alice.session_id, 0)).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    let status: Status = server.get("/v3/status").send().await.msgpack();
    assert_eq!(status.room_count, 0, "誰もいない部屋が残っています");
}
#[tokio::test]
async fn removes_an_abandoned_room_without_any_sync() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    create_room(&server, 2).await;

    // 誰もその部屋に触れない。sync が来なくても、保持期間を過ぎれば掃除される。
    tokio::time::sleep(Duration::from_millis(400)).await;

    let status: Status = server.get("/v3/status").send().await.msgpack();
    assert_eq!(status.room_count, 0, "放棄された部屋が残っています");
}

#[tokio::test]
async fn resending_does_not_duplicate_an_event() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let id = Uuid::now_v7().to_string();
    let mut body = sync_request(&alice.session_id, 0);
    body.actions = vec![OutEvent {
        id: id.clone(),
        kind: "attack".to_owned(),
        data: "A".to_owned(),
    }];

    // 応答を受け取れなかったクライアントを模す。預け入れは済んでいる。
    let lost = tokio::spawn({
        let server = server.clone();
        let body = sync_request(&alice.session_id, 0);
        let mut body = body;
        body.actions = vec![OutEvent {
            id: id.clone(),
            kind: "attack".to_owned(),
            data: "A".to_owned(),
        }];
        async move { post_sync(&server, &body).await }
    });
    tokio::time::sleep(Duration::from_millis(10)).await;
    lost.abort();

    // 窓が締まるのを待ってから、同じ本文で再送する。
    tokio::time::sleep(Duration::from_millis(80)).await;
    post_sync(&server, &body).await;

    tokio::time::sleep(Duration::from_millis(80)).await;
    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .msgpack();

    let count = synced.actions.iter().filter(|e| e.id == id).count();
    assert_eq!(count, 1, "同じ event が {count} 回届きました");
}

#[tokio::test]
async fn events_from_one_sender_keep_their_order() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 2).await;

    let mut body = sync_request(&alice.session_id, 0);
    body.actions = (0..5).map(|i| event("attack", &i.to_string())).collect();
    let sent: Vec<String> = body.actions.iter().map(|e| e.id.clone()).collect();
    let synced: Synced = post_sync(&server, &body).await.expect_ok();

    let received: Vec<String> = synced.actions.iter().map(|e| e.id.clone()).collect();
    assert_eq!(received, sent, "同一送信者内の順序が崩れています");
    let data: Vec<&str> = synced.actions.iter().map(|e| e.data.as_str()).collect();
    assert_eq!(data, ["0", "1", "2", "3", "4"], "中身が入れ替わっています");
}

#[tokio::test]
async fn room_users_are_sorted_by_id_with_the_owner_first() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    let alice = create_room(&server, 4).await;
    join_room(&server, &alice.code(), "Bob").await;
    join_room(&server, &alice.code(), "Charlie").await;

    let synced: Synced = post_sync(&server, &sync_request(&alice.session_id, 0))
        .await
        .expect_ok();

    let ids: Vec<&str> = synced.room_users.iter().map(|u| u.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "ユーザー一覧が ID 昇順ではありません");
    assert_eq!(
        synced.room_users[0].id, alice.user_id,
        "先頭が部屋主ではありません"
    );
    assert_eq!(synced.room_users[0].name, "Alice");
}

#[tokio::test]
async fn a_response_does_not_arrive_before_the_deadline() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };
    // 部屋の人数ぶん全員が預けても、窓の終わりまで締め切ってはならない。
    let alice = create_room(&server, 2).await;
    let bob = join_room(&server, &alice.code(), "Bob").await;

    let (from_alice, from_bob) = (
        sync_request(&alice.session_id, 0),
        sync_request(&bob.session_id, 0),
    );
    let started = std::time::Instant::now();
    let both = tokio::join!(
        post_sync(&server, &from_alice),
        post_sync(&server, &from_bob),
    );
    let elapsed = started.elapsed();

    assert_eq!(both.0.status, StatusCode::OK);
    assert_eq!(both.1.status, StatusCode::OK);
    assert!(
        elapsed >= config().tick / 2,
        "全員到着で早く締め切っています: {elapsed:?}"
    );
}

#[tokio::test]
async fn a_session_expires_with_its_room() {
    let Some(server) = TestServer::with_config(Config {
        lobby_lifetime: Duration::from_millis(50),
        ..config()
    })
    .await
    else {
        return;
    };
    let alice = create_room(&server, 2).await;
    tokio::time::sleep(Duration::from_millis(80)).await;

    let reply = post_sync(&server, &sync_request(&alice.session_id, 0)).await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.error_code(), "invalid_session");
}

#[tokio::test]
async fn a_started_game_expires_on_its_own_lifetime() {
    let Some(server) = TestServer::with_config(Config {
        // ロビーは長く、ゲームは短く。開始後の期限が使われることを分ける。
        lobby_lifetime: Duration::from_secs(60),
        game_lifetime: Duration::from_millis(50),
        ..config()
    })
    .await
    else {
        return;
    };
    let alice = create_room(&server, 2).await;
    server
        .post(
            "/v3/room/start",
            &Start {
                session_id: alice.session_id.clone(),
            },
        )
        .send()
        .await;
    tokio::time::sleep(Duration::from_millis(80)).await;

    let reply = post_sync(&server, &sync_request(&alice.session_id, 0)).await;

    assert_eq!(
        reply.status,
        StatusCode::UNAUTHORIZED,
        "ゲームの期限が効いていません"
    );
}
