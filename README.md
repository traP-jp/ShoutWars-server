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

push と pull request では `.github/workflows/ci.yml` が同じものを回す。あわせて Dockerfile のビルドと `cargo audit` も確認する。

## 環境変数

| 変数 | 既定値 | 内容 |
|---|---|---|
| `PORT` | `7468` | ポート番号 |
| `PASSWORD` | なし | 設定時は `Authorization: Bearer` を要求する |
| `ROOM_LIMIT` | `100` | 部屋数の上限 |
| `ROOM_MEMORY_LIMIT` | `4` | 部屋ごとに保持するイベントの合計 (MiB) |

いずれも起動時に検証する。解釈できない値や範囲外の値は、既定値へ黙ってフォールバックせず、エラーで起動を中止する。

メモリの消費量は `ROOM_LIMIT` × `ROOM_MEMORY_LIMIT` で頭打ちになる。この積が、サーバーに割り当てたメモリに収まるように決めること。

部屋の寿命は環境変数にしていない。クライアントがこの値に依存しており、配備ごとに変えると黙って壊れるためである ([docs/protocol.md](docs/protocol.md))。

tick の幅とレコードの保持数も環境変数にしていないが、こちらは配備によって変える理由が無いためである。仕様書は数値を定めていないので、必要になれば環境変数にできる。

## API 仕様

[docs/protocol.md](docs/protocol.md) を参照。
