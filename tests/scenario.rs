//! 複数人が周期をずらしながら同期し続ける、通しのシナリオ。
//!
//! 個々の規則を確かめるテストとは狙いが違う。レコードの締め切り、不在と復帰、カーソルの進行、
//! `applied` の照合は、どれも単体では正しく見えて、絡み合ったときに初めて壊れる。

mod common;

use std::{collections::HashMap, time::Duration};

use common::TestServer;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shoutwars_server::config::Config;
use uuid::Uuid;

/// 参加者。3 人にすると、報告イベントを他人同士で突き合わせられる。
const NAMES: [&str; 3] = ["Alice", "Bob", "Charlie"];
const ROUNDS: usize = 30;

/// 実時間を待つため、仕様の 100 ms より大幅に短くする。
fn config() -> Config {
    Config {
        tick: Duration::from_millis(30),
        ..Config::default()
    }
}

/// 送信の間隔。乱数にすると失敗が再現しなくなるので、規則的にばらつかせる。
///
/// **tick 幅より短くしてはならない。** 短いと、前回預けたレコードがまだ開いている
/// うちに次を送ることになり、403 already_synced が返る。仕様の「`tick_ms` 周期で送信する」は、
/// クライアントが守るべき制約である。
///
/// 幅を持たせてあるのは、締め切りに間に合う回と間に合わない回を混ぜるため。不在の判定と、
/// 不在だった人が次のレコードから戻る経路を通す。
fn interval(round: usize, player: usize) -> Duration {
    let tick = config().tick;
    tick + Duration::from_millis(((round * 7 + player * 13) % 15 + 2) as u64)
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

#[derive(Debug, Serialize)]
struct Sync {
    session_id: String,
    next_tick: u64,
    applied: u64,
    reports: Vec<OutEvent>,
    actions: Vec<OutEvent>,
}

#[derive(Debug, Serialize)]
struct OutEvent {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct Joined {
    session_id: String,
    user_id: String,
    #[serde(default)]
    name: String,
    next_tick: u64,
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
}

#[derive(Debug, Deserialize)]
struct InEvent {
    id: String,
    from: String,
    #[serde(rename = "type")]
    kind: String,
    data: String,
}

fn sorted_names() -> Vec<&'static str> {
    let mut names = NAMES.to_vec();
    names.sort_unstable();
    names
}

/// 1 人ぶんの通しの結果。
struct Played {
    name: String,
    user_id: String,
    /// 受け取った確認イベントの ID を、受け取った順に並べたもの。
    actions: Vec<String>,
    /// 受け取った報告イベントを、送信者ごとに受信順で分けたもの。
    reports_by_sender: HashMap<String, Vec<String>>,
    /// 自分が送った確認イベントの ID。
    sent: Vec<String>,
}

/// 1 人を最後まで走らせ、応答ごとに不変条件を確かめる。
async fn play(server: TestServer, player: usize, member: Joined) -> Played {
    let mut cursor = member.next_tick;
    let mut applied = 0_u64;
    let mut result = Played {
        name: member.name.clone(),
        user_id: member.user_id.clone(),
        actions: Vec::new(),
        reports_by_sender: HashMap::new(),
        sent: Vec::new(),
    };

    // 送り終えた後も、他の人の最後のイベントが届くまで空の同期を続ける。
    let expected = NAMES.len() * ROUNDS;
    for round in 0..ROUNDS + 40 {
        tokio::time::sleep(interval(round, player)).await;

        let mut body = Sync {
            session_id: member.session_id.clone(),
            next_tick: cursor,
            applied,
            reports: Vec::new(),
            actions: Vec::new(),
        };
        if round < ROUNDS {
            let action = OutEvent {
                id: Uuid::now_v7().to_string(),
                kind: "attack".to_owned(),
                data: format!("{}:{round}", member.name),
            };
            result.sent.push(action.id.clone());
            body.actions.push(action);
            body.reports.push(OutEvent {
                id: Uuid::now_v7().to_string(),
                kind: "position".to_owned(),
                data: format!("{}:{round}", member.name),
            });
        }

        let reply = server.post("/v3/room/sync", &body).send().await;
        assert_eq!(reply.status, StatusCode::OK, "{} の同期が失敗", member.name);
        let synced: Synced = reply.msgpack();

        assert!(
            synced.next_tick > cursor,
            "{} のカーソルが進みませんでした: {} -> {}",
            member.name,
            cursor,
            synced.next_tick
        );
        assert!(
            !synced.desync,
            "{} が正常に同期しているのに desync と判定されました",
            member.name
        );
        assert!(!synced.started, "誰も開始していません");
        assert_eq!(synced.room_users.len(), NAMES.len(), "人数が変わりました");
        assert!(
            synced.room_users.iter().any(|u| u.id == member.user_id),
            "{} が一覧にいません",
            member.name
        );
        let mut names: Vec<&str> = synced.room_users.iter().map(|u| u.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, sorted_names(), "参加者の顔ぶれが変わりました");

        for event in &synced.reports {
            assert_ne!(
                event.from, member.user_id,
                "自分の報告が返ってきました ({})",
                member.name
            );
            assert_eq!(event.kind, "position", "報告の type が書き換わりました");
            assert!(!event.data.is_empty(), "報告の data が失われました");
        }
        for event in &synced.actions {
            assert_eq!(event.kind, "attack", "確認の type が書き換わりました");
            assert!(!event.data.is_empty(), "確認の data が失われました");
        }

        cursor = synced.next_tick;
        applied += (synced.reports.len() + synced.actions.len()) as u64;
        for event in &synced.actions {
            result.actions.push(event.id.clone());
        }
        for event in &synced.reports {
            result
                .reports_by_sender
                .entry(event.from.clone())
                .or_default()
                .push(event.id.clone());
        }

        if round >= ROUNDS && result.actions.len() >= expected {
            break;
        }
    }
    result
}

#[tokio::test]
async fn three_players_stay_in_sync() {
    let Some(server) = TestServer::with_config(config()).await else {
        return;
    };

    let owner: Joined = server
        .post(
            "/v3/room/create",
            &Create {
                version: "1.0".to_owned(),
                user: UserName {
                    name: NAMES[0].to_owned(),
                },
                size: NAMES.len(),
            },
        )
        .send()
        .await
        .msgpack();
    // create の応答の `name` は部屋番号。参加者名で上書きする前に控える。
    let room_number = owner.name.clone();
    let mut members = vec![Joined {
        name: NAMES[0].to_owned(),
        ..owner
    }];
    for name in &NAMES[1..] {
        let joined: Joined = server
            .post(
                "/v3/room/join",
                &Join {
                    version: "1.0".to_owned(),
                    name: room_number.clone(),
                    user: UserName {
                        name: (*name).to_owned(),
                    },
                },
            )
            .send()
            .await
            .msgpack();
        members.push(Joined {
            name: (*name).to_owned(),
            ..joined
        });
    }
    let played = {
        let mut tasks = Vec::new();
        for (player, member) in members.into_iter().enumerate() {
            let server = server.clone();
            tasks.push(tokio::spawn(play(server, player, member)));
        }
        let mut played = Vec::new();
        for task in tasks {
            played.push(task.await.expect("参加者の同期が終わりませんでした"));
        }
        played
    };

    let sent: usize = played.iter().map(|p| p.sent.len()).sum();
    assert_eq!(sent, NAMES.len() * ROUNDS);

    // 保証 1。全員が同一の集合を同一順序で受け取る。
    for other in &played[1..] {
        assert_eq!(
            other.actions, played[0].actions,
            "{} と {} で確認イベントの順序が違います",
            played[0].name, other.name
        );
    }
    assert_eq!(
        played[0].actions.len(),
        sent,
        "送った確認イベントが全員へ届いていません"
    );

    // 報告イベントは送信者に返らないため、第三者どうしで突き合わせる。
    for sender in &played {
        let seen: Vec<&Vec<String>> = played
            .iter()
            .filter(|p| p.user_id != sender.user_id)
            .map(|p| {
                p.reports_by_sender
                    .get(&sender.user_id)
                    .unwrap_or_else(|| panic!("{} の報告を誰も受け取っていません", sender.name))
            })
            .collect();
        assert_eq!(
            seen[0], seen[1],
            "{} の報告の順序が受信者によって違います",
            sender.name
        );
    }
}
