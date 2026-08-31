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

#[test]
fn 解釈できない値では起動しない() {
    let (code, stderr) = 起動を試す("ROOM_LIMIT", "abc");

    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("ROOM_LIMIT"),
        "原因が分からない出力: {stderr}"
    );
}

#[test]
fn 範囲外の値では起動しない() {
    let (code, stderr) = 起動を試す("ROOM_LIMIT", "0");

    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("ROOM_LIMIT"),
        "原因が分からない出力: {stderr}"
    );
}
