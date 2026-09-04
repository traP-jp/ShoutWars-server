//! 環境変数の検証 (README の環境変数表)。
//!
//! 不正な設定で黙って既定値へ落ちるとデプロイの事故に気づけないため、
//! 起動を中止することをバイナリごと動かして確かめる。

use std::process::Command;

fn try_start(name: &str, value: &str) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_shoutwars-server"))
        .env(name, value)
        .output()
        .expect("サーバーを起動できません");
    (
        output.status.code(),
        String::from_utf8(output.stderr).expect("stderr が UTF-8 ではありません"),
    )
}

fn assert_refuses_to_start(cases: &[(&str, &str)]) {
    for (name, value) in cases {
        let (code, stderr) = try_start(name, value);
        assert_eq!(code, Some(1), "{name}={value} で起動してしまいました");
        assert!(
            stderr.contains(name),
            "{name}={value} の出力から原因が分かりません: {stderr}"
        );
    }
}

#[test]
fn refuses_to_start_on_unparsable_value() {
    assert_refuses_to_start(&[
        ("PORT", "ななよんろくはち"),
        ("ROOM_LIMIT", "abc"),
        ("ROOM_MEMORY_LIMIT", "1.5"),
        ("ROOM_LIMIT", ""),
    ]);
}

#[test]
fn refuses_to_start_on_out_of_range_value() {
    assert_refuses_to_start(&[("ROOM_LIMIT", "0"), ("ROOM_MEMORY_LIMIT", "0")]);
}
