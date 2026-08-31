//! サーバーを HTTP 越しに叩くための足場。
//!
//! ハンドラを直接呼ばず実際に TCP へ束ねるのは、クライアントと同じ視点で検証するため。
//! 内部の構造ではなく `docs/protocol.md` に書かれた振る舞いだけを対象にする。
//!
//! 環境変数 `TEST_SERVER_URL` (例: `https://example.com`) を設定すると、
//! ローカルで起動する代わりにそのサーバーへ同じテストを流す。バージョンの
//! パス (`/v3`) は含めない。設定に依存するテストは、こちらの設定を制御
//! できないため [`TestServer::with_config`] が `None` を返して省略される。
//! そのサーバーがパスワードを要求する場合は `TEST_SERVER_PASSWORD` に指定する。

// 統合テストは 1 ファイルにつき 1 クレートとしてビルドされるため、使っていない
// ファイル側では未使用に見えてしまう。どのクレートで使われるかは一定しないので、
// 充足を要求する expect ではなく allow を使う。
#![allow(dead_code, reason = "テストクレートごとに使う項目が異なる")]

use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
};

use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use shoutwars_server::config::Config;
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct TestServer {
    base_url: String,
    http: reqwest::Client,
    /// 全リクエストに既定で付けるパスワード。`Request::bearer` で上書きできる。
    password: Option<String>,
}

/// 外部サーバーを向ける場合の宛先。
fn 外部サーバー() -> Option<String> {
    env::var("TEST_SERVER_URL")
        .ok()
        .map(|url| url.trim_end_matches('/').to_owned())
}

impl TestServer {
    /// サーバーの設定に依存しないテスト用。外部サーバーが指定されていればそこへ向ける。
    pub async fn start() -> Self {
        match 外部サーバー() {
            Some(base_url) => Self::at(base_url),
            None => Self::spawn(&Config::default()).await,
        }
    }

    /// 設定に依存するテスト用。
    ///
    /// 外部サーバーの設定はこちらから決められないため、その場合は `None` を返す。
    /// 呼び出し側は `let Some(server) = ... else { return }` で省略する。
    pub async fn with_config(config: Config) -> Option<Self> {
        if 外部サーバー().is_some() {
            return None;
        }
        Some(Self::spawn(&config).await)
    }

    /// 空きポートを確保して起動する。`port` の設定は無視する。
    ///
    /// テストが終わるとランタイムごと落ちるため、明示的な停止は要らない。
    async fn spawn(config: &Config) -> Self {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .expect("空きポートを確保できません");
        let port = listener
            .local_addr()
            .expect("待ち受けアドレスを取得できません")
            .port();

        tokio::spawn(shoutwars_server::serve(
            listener,
            shoutwars_server::app(config),
            std::future::pending(),
        ));

        Self {
            base_url: format!("http://{}:{port}", Ipv4Addr::LOCALHOST),
            http: reqwest::Client::new(),
            password: None,
        }
    }

    fn at(base_url: String) -> Self {
        Self {
            base_url,
            http: reqwest::Client::new(),
            password: env::var("TEST_SERVER_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
        }
    }

    pub fn get(&self, path: &str) -> Request {
        self.request(Method::GET, path)
    }

    /// 本文を MessagePack で符号化して送る。
    pub fn post(&self, path: &str, body: &impl Serialize) -> Request {
        let mut encoded = Vec::new();
        let mut serializer = rmp_serde::Serializer::new(&mut encoded)
            .with_struct_map()
            .with_human_readable();
        body.serialize(&mut serializer)
            .expect("本文を符号化できません");
        Request(
            self.request(Method::POST, path)
                .0
                .header(reqwest::header::CONTENT_TYPE, "application/msgpack")
                .body(encoded),
        )
    }

    fn request(&self, method: Method, path: &str) -> Request {
        let builder = self
            .http
            .request(method, format!("{}{path}", self.base_url));
        Request(match &self.password {
            Some(password) => builder.bearer_auth(password),
            None => builder,
        })
    }
}

#[derive(Debug)]
pub struct Request(reqwest::RequestBuilder);

impl Request {
    #[must_use]
    pub fn bearer(self, password: &str) -> Self {
        Self(self.0.bearer_auth(password))
    }

    /// ヘッダを直に指定する。`Authorization` の書式そのものを試すために使う。
    #[must_use]
    pub fn header(self, name: &str, value: &str) -> Self {
        Self(self.0.header(name, value))
    }

    pub async fn send(self) -> Reply {
        let response = self.0.send().await.expect("リクエストを送れません");
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .map(|value| {
                value
                    .to_str()
                    .expect("Content-Type が ASCII ではありません")
            })
            .map(ToOwned::to_owned);
        let body = response.bytes().await.expect("本文を受け取れません");
        Reply {
            status,
            content_type,
            body: body.to_vec(),
        }
    }
}

#[derive(Debug)]
pub struct Reply {
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl Reply {
    /// 本文を MessagePack として読む。Content-Type も併せて確かめる。
    pub fn msgpack<T: DeserializeOwned>(&self) -> T {
        assert_eq!(
            self.content_type.as_deref(),
            Some("application/msgpack"),
            "Content-Type が仕様と異なります"
        );
        // サーバーと同じ表現を選ぶ。UUID は 16 バイトの配列ではなく文字列で流れる。
        let mut deserializer =
            rmp_serde::Deserializer::from_read_ref(&self.body).with_human_readable();
        T::deserialize(&mut deserializer).expect("本文を MessagePack として読めません")
    }

    /// エラー本文を読み、`code` を返す。
    ///
    /// `message` が空でないことも確かめる。UI に表示される値であり、
    /// 空だとクライアントが何も出せなくなる。
    pub fn error_code(&self) -> String {
        assert!(
            self.status.is_client_error() || self.status.is_server_error(),
            "エラーではないレスポンスです: {}",
            self.status
        );
        let body: ErrorBody = self.msgpack();
        assert!(!body.error.message.is_empty(), "message が空です");
        body.error.code
    }
}

#[derive(Debug, Deserialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}
