# ShoutWars バックエンド

© 2024 traP Community  
ライセンス: [MIT License](LICENSE)

traP ワンマンソン 2024 レジェンドクリエイターズのゲーム

クライアント: [traP-jp/ShoutWars](https://github.com/traP-jp/ShoutWars)

**通信仕様: [docs/protocol.md](docs/protocol.md)**

## 起動方法

TODO

## ビルド

TODO

## 環境変数

| 変数 | 既定値 | 内容 |
|---|---|---|
| `PORT` | `7468` | ポート番号 |
| `PASSWORD` | なし | 設定時は `Authorization: Bearer` を要求する |
| `ROOM_LIMIT` | `100` | 部屋数の上限 |
| `LOBBY_LIFETIME` | `10` 分 | 各部屋のロビーの制限時間 |
| `GAME_LIFETIME` | `20` 分 | 各部屋のゲームの制限時間 |

いずれも起動時に検証する。解釈できない値や範囲外の値は、既定値へ黙ってフォールバックせず、エラーで起動を中止する。

`ROOM_LIMIT` は、サーバーが同時に処理できる部屋数を超えて設定してはならない。超える値が指定された場合は、起動時に警告するか拒否する。

## API 仕様

[docs/protocol.md](docs/protocol.md) を参照。
