# ShoutWars バックエンド

© 2024 traP Community  
ライセンス: [MIT License](LICENSE)

traP ワンマンソン 2024 レジェンドクリエイターズのゲーム

クライアント: [traP-jp/ShoutWars](https://github.com/traP-jp/ShoutWars)

**通信仕様: [docs/protocol.md](docs/protocol.md)**

## ビルド

[rustup](https://rustup.rs/) が入っていれば、ツールチェインは `rust-toolchain.toml` に従って自動で用意されます。

```sh
cargo build --release
```

コンテナで動かす場合は `Dockerfile` を使います。デプロイ先と同じものが手元で再現できます。

```sh
docker build -t shoutwars-server .
docker run -p 7468:7468 shoutwars-server
```

## 起動方法

```sh
cargo run --release
```

ビルド済みのバイナリは `target/release/shoutwars-server` にあります。

## 開発

```sh
cargo clippy --all-targets   # Lint
cargo fmt                    # 整形
cargo test                   # テスト
```

テストはサーバーを空きポートで起動し、HTTP 越しに叩いて仕様どおりの応答かを確かめる。`TEST_SERVER_URL` を指定すると、代わりにそのサーバーへ同じテストを流す。デプロイ先の確認に使える。

```sh
TEST_SERVER_URL=https://example.com TEST_SERVER_PASSWORD=... cargo test
```

このとき、サーバーの設定に依存するテストは設定を制御できないため実行されない。

未実装の機能に対するテストは `#[ignore]` を付けてある。`cargo test -- --ignored` で残りを確認できる。

## 環境変数

| 変数 | 既定値 | 内容 |
|---|---|---|
| `PORT` | `7468` | ポート番号 |
| `PASSWORD` | なし | 設定時は `Authorization: Bearer` を要求する |
| `ROOM_LIMIT` | `100` | 部屋数の上限 |
| `LOBBY_LIFETIME` | `10` 分 | 各部屋のロビーの制限時間 |
| `GAME_LIFETIME` | `20` 分 | 各部屋のゲームの制限時間 |
| `TICK_MS` | `100` ミリ秒 | tick の幅。参加時の応答でクライアントへ通知する |
| `RECORD_RETENTION` | `100` | 部屋ごとに保持する同期レコードの数 |

いずれも起動時に検証する。解釈できない値や範囲外の値は、既定値へ黙ってフォールバックせず、エラーで起動を中止する。

`TICK_MS` と `RECORD_RETENTION` を短くすると、時間に依存する振る舞いを実時間を待たずにテストできる。運用時に既定値から変える必要は無い。

`ROOM_LIMIT` は、サーバーが同時に処理できる部屋数を超えて設定してはならない。超える値が指定された場合は、起動時に警告するか拒否する。

## デプロイ

NeoShowcase 上で 2 つ動いています。ブランチへの push で自動デプロイされ、ビルドはリポジトリ直下の `Dockerfile` で行われます。

| 環境 | ブランチ | 環境変数 |
|---|---|---|
| 本番 | `main` | `ROOM_LIMIT=20` |
| 開発 | `develop` | `PASSWORD` 設定あり、`ROOM_LIMIT=10` |

## API 仕様

[docs/protocol.md](docs/protocol.md) を参照。
