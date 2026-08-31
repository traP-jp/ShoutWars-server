//! 環境変数の検証 (README の環境変数表)。
//!
//! 不正な設定で黙って既定値へ落ちるとデプロイの事故に気づけないため、
//! 起動を中止することをバイナリごと動かして確かめる。

use std::process::Command;

fn 起動を試す(name: &str, value: &str) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_shoutwars-server"))
        .env(name, value)
        .output()
        .expect("サーバーを起動できません");
    (
        output.status.code(),
        String::from_utf8(output.stderr).expect("stderr が UTF-8 ではありません"),
    )
}

fn 起動しないことを確かめる(cases: &[(&str, &str)]) {
    for (name, value) in cases {
        let (code, stderr) = 起動を試す(name, value);
        assert_eq!(code, Some(1), "{name}={value} で起動してしまいました");
        assert!(
            stderr.contains(name),
            "{name}={value} の出力から原因が分かりません: {stderr}"
        );
    }
}

#[test]
fn 解釈できない値では起動しない() {
    起動しないことを確かめる(&[
        ("PORT", "ななよんろくはち"),
        ("ROOM_LIMIT", "abc"),
        ("TICK_MS", "1.5"),
        ("LOBBY_LIFETIME", ""),
    ]);
}

#[test]
fn 範囲外の値では起動しない() {
    起動しないことを確かめる(&[
        ("ROOM_LIMIT", "0"),
        ("TICK_MS", "0"),
        ("RECORD_RETENTION", "0"),
        ("GAME_LIFETIME", "0"),
    ]);
}
