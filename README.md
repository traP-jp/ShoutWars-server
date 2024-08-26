# ShoutWars (仮) バックエンド

ワンマンソン 2024 レジェンドクリエイターズのゲーム

クライアント: [traP-jp/ShoutWars](https://github.com/traP-jp/ShoutWars)

## 起動方法

### 本番環境

Ubuntu の Docker イメージなどでは

```sh
apt update
apt install -y cmake g++ git wget
cmake .
make
```

でビルドできます。

```sh
./ShoutWars_server
```

でサーバーを起動します。

### 開発環境

CMake, C++, Git がインストールされている環境で

```sh
mkdir -p cmake-build-debug
cd cmake-build-debug
cmake ..
make
cd ..
```

でビルドします。CLion などの IDE では CMake Application を設定すればビルドできます。

```sh
./cmake-build-debug/ShoutWars_server
```

でサーバーを起動します。

## 環境変数

- `PORT`: ポート番号 (デフォルト: `7468`)
- `PASSWORD`: パスワード (デフォルト: なし)
- `ROOM_LIMIT`: 部屋数の上限 (デフォルト: `100`)
- `LOBBY_LIFETIME`: 各部屋のロビーの制限時間 (デフォルト: `10` 分)
- `GAME_LIFETIME`: 各部屋のゲームの制限時間 (デフォルト: `20` 分)

## API 仕様

Request と Response の body は MessagePack 形式でやり取りします。  
Content-Type は `application/msgpack` とします。  
エラーが発生した場合は `4xx` や `5xx` のステータスコードと以下の形式でエラーメッセージを返します。

```msgpack
{
  "error": string // エラーメッセージ
}
```

環境変数 `PORT` でポート番号を指定できます。デフォルトは `7468` です。  
環境変数 `PASSWORD` が設定されている場合、リクエストヘッダの `Authorization` に `Bearer ${PASSWORD}` を指定する必要があります。

エンドポイントは `/v2` です。`uuid` は UUIDv7 で生成された文字列としています。  
また、それ以外のプリミティブでない型はクライアント側の実装に依存します。

### `POST /room/create`

部屋を作成する。

#### Request

```msgpack
{
  "version": string, // クライアントのバージョン
  "user": {
    "name": string // ユーザー名 (32 文字以内)
  },
  "size": number // 部屋の人数 (2~4 の整数)
}
```

#### Response

```msgpack
{
  "session_id": uuid, // セッション ID
  "user_id": uuid, // 自分のユーザー ID
  "id": uuid, // 部屋 ID
  "name": string // 部屋番号 (6 桁の数字)
}
```

### `POST /room/join`

部屋に参加する。

#### Request

```msgpack
{
  "version": string, // クライアントのバージョン
  "name": string, // 部屋番号 (6 桁の数字)
  "user": {
    "name": string // ユーザー名 (32 文字以内)
  }
}
```

#### Response

```msgpack
{
  "session_id": uuid, // セッション ID
  "id": uuid, // 部屋 ID
  "user_id": uuid, // 自分のユーザー ID
  "room_info": RoomInfo // 部屋情報
}
```

### `POST /room/sync`

部屋の情報やゲームの状態を同期する。

部屋の全ユーザーのリクエストが揃ってからレスポンスを返します。クライアントはこのレスポンスを受け取るたびに 100 ms 後に次の同期をリクエストしてください。  
ただし、最初のリクエストから 50 ms (遅れたユーザーは + 200 ms) 以上経過したら即座にレスポンスを返し、遅れたユーザーのイベントは次の同期に持ち越します。  
10 秒間リクエストの無いユーザーは脱落となります。

#### Request

```msgpack
{
  "session_id": uuid, // セッション ID
  "room_info": RoomInfo, // 部屋情報
  "reports": [{ // 報告イベント
    "id": uuid, // イベント ID
    "type": string, // イベントの種類
    "event": Event // イベントの内容
  }],
  "actions": [{ // 確認イベント
    "id": uuid, // イベント ID
    "type": string, // イベントの種類
    "event": Event // イベントの内容
  }]
}
```

#### Response

```msgpack
{
  "id": uuid, // 同期 ID
  "reports": [{ // 報告イベント (id でソートされます)
    "id": uuid, // イベント ID
    "sync_id": uuid, // このイベントが送信されるはずだった同期 ID (今回の同期 ID と異なる場合のみ)
    "from": uuid, // 送信元のユーザー ID
    "type": string, // イベントの種類
    "event": Event // イベントの内容
  }],
  "actions": [{ // 確認イベント (id でソートされます)
    "id": uuid, // イベント ID
    "sync_id": uuid, // このイベントが送信されるはずだった同期 ID (今回の同期 ID と異なる場合のみ)
    "from": uuid, // 送信元のユーザー ID
    "type": string, // イベントの種類
    "event": Event // イベントの内容
  }],
  "room_users": [{ // 部屋のユーザー (最初が部屋主)
    "id": uuid, // ユーザー ID
    "name": string // ユーザー名
  }]
}
```

レスポンスを返してから 100 ms 以内にリクエストが来た場合は即座に `429 Too Many Requests` を返します。

### `POST /room/start`

ゲームを開始する。

このリクエストは部屋主のみが送信できます。

#### Request

```msgpack
{
  "session_id": uuid // セッション ID
}
```

#### Response

```msgpack
{}
```

### `GET /status`

サーバーのステータスを取得する。

#### Response

```msgpack
{
  "room_count": number, // 部屋数
  "room_limit": number // 部屋数の上限
}
```
