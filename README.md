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

## API 仕様

Request と Response の body は MessagePack 形式でやり取りします。  
Content-Type は `application/msgpack` とします。  
エラーが発生した場合は `4xx` や `5xx` のステータスコードと以下の形式でエラーメッセージを返します。

```msgpack
{
  "error": string, // エラーメッセージ
}
```

環境変数 `PORT` でポート番号を指定できます。デフォルトは `7468` です。  
環境変数 `PASSWORD` が設定されている場合、リクエストヘッダの `Authorization` に `Bearer ${PASSWORD}` を指定する必要があります。

エンドポイントは `/v0` です。`uuid` は UUIDv7 で生成された文字列としています。  
また、それ以外のプリミティブでない型はクライアント側の実装に依存します。

### `POST /room/create`

部屋を作成する。

#### Request

```msgpack
{
  "version": string, // クライアントのバージョン
  "user": {
    "name": string, // ユーザー名 (32 文字以内)
  },
  "size": number, // 部屋の人数 (2~4 の整数)
}
```

#### Response

```msgpack
{
  "session_id": uuid, // セッション ID
  "user_id": uuid, // 自分のユーザー ID
  "id": uuid, // 部屋 ID
}
```

サーバーの部屋数が上限に達している場合は `403 Forbidden` を返します。  
部屋数の上限は環境変数 `ROOM_LIMIT` で指定できます。デフォルトは `100` です。

### `POST /room/join`

部屋に参加する。

#### Request

```msgpack
{
  "version": string, // クライアントのバージョン
  "id": uuid, // 部屋 ID
  "user": {
    "name": string, // ユーザー名 (32 文字以内)
  },
}
```

#### Response

```msgpack
{
  "session_id": uuid, // セッション ID
  "user_id": uuid, // 自分のユーザー ID
  "room": Room, // 部屋情報
}
```

部屋が存在しない場合は `404 Not Found` を返します。  
また、部屋は存在するが入れない場合は `403 Forbidden` を返します。

### `POST /room/sync`

部屋の情報やゲームの状態を同期する。

#### Request

```msgpack
{
  "session_id": uuid, // セッション ID
  "reports": [{ // 報告イベント
    "id": uuid, // イベント ID
    "type": string, // イベントの種類
    "event": Event, // イベントの内容
  }],
  "actions": [{ // 確認イベント
    "id": uuid, // イベント ID
    "type": string, // イベントの種類
    "event": Event, // イベントの内容
  }],
}
```

#### Response

部屋の全ユーザーのリクエストが揃ってからレスポンスを返します。前回のリクエストから 5 秒以上経過したユーザーは脱落となります。

```msgpack
{
  "reports": [{ // 報告イベント (id でソートされます)
    "id": uuid, // イベント ID
    "from": uuid, // 送信元のユーザー ID
    "type": string, // イベントの種類
    "event": Event, // イベントの内容
  }],
  "actions": [{ // 確認イベント (id でソートされます)
    "id": uuid, // イベント ID
    "from": uuid, // 送信元のユーザー ID
    "type": string, // イベントの種類
    "event": Event, // イベントの内容
  }],
  "drop_users": [uuid], // 脱落したユーザーの ID
}
```

セッション ID が不正な場合は `401 Unauthorized` を返します。

### `GET /status`

サーバーのステータスを取得する。

#### Response

```msgpack
{
  "room_count": number, // 部屋数
  "room_limit": number, // 部屋数の上限
}
```
