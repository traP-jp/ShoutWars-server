//! 部屋の登録簿。
//!
//! 単一スレッドで動かすが、ハンドラは `Send` を要求されるため `std::sync::Mutex` で包む。
//! 標準の `MutexGuard` は `!Send` なので、ロックを持ったまま `.await` すると
//! コンパイルが通らない。旧実装で起きていた種類の不具合が型で防がれる。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use rmpv::Value;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    config::Config,
    error::Error,
    record::Incoming,
    room::{Room, RoomNumber, User},
};

/// セッションが指す先 (§3.6)。
#[derive(Debug, Clone, Copy)]
pub struct Session {
    pub room: RoomNumber,
    pub user: Uuid,
}

#[derive(Debug)]
pub struct Rooms {
    config: Arc<Config>,
    by_number: HashMap<RoomNumber, Room>,
    sessions: HashMap<Uuid, Session>,
}

/// 部屋番号の引き直しの上限 (§6.1)。無いと、番号が埋まってきたときに際限なく回る。
const NUMBERING_ATTEMPTS: usize = 32;

impl Rooms {
    /// 期限切れの部屋を取り除く (§3.1)。
    fn sweep(&mut self) {
        let now = Instant::now();
        let config = &self.config;
        let sessions = &mut self.sessions;
        self.by_number.retain(|_, room| {
            if !room.is_expired(config, now) {
                return true;
            }
            for user in &room.users {
                sessions.remove(&user.session_id);
            }
            tracing::info!(number = %room.number, "期限切れの部屋を削除しました");
            false
        });
    }

    pub fn count(&mut self) -> usize {
        self.sweep();
        self.by_number.len()
    }

    /// 部屋を作る (§4.2)。
    ///
    /// # Errors
    /// 部屋数が上限に達している場合、または番号を採れなかった場合。
    pub fn create(&mut self, version: String, owner: User, size: usize) -> Result<&Room, Error> {
        self.sweep();
        if self.by_number.len() >= self.config.room_limit {
            return Err(Error::RoomLimitReached);
        }
        let number = self.take_number()?;
        let session = Session {
            room: number,
            user: owner.id,
        };
        self.sessions.insert(owner.session_id, session);
        let room = Room::new(number, version, owner, size)?;
        tracing::info!(id = %room.id, %number, size, "部屋を作成しました");
        Ok(self.by_number.entry(number).or_insert(room))
    }

    /// 部屋に参加する (§4.3)。
    ///
    /// # Errors
    /// 部屋が無い・期限切れ・バージョン不一致・開始済み・満員・名前が長すぎる場合。
    pub fn join(
        &mut self,
        number: RoomNumber,
        version: &str,
        name: String,
    ) -> Result<Joined, Error> {
        self.sweep();
        // 期限切れは sweep で消えているため、ここに残っていれば有効な部屋である (§3.1)。
        let room = self.by_number.get_mut(&number).ok_or(Error::RoomNotFound)?;
        if room.version != version {
            return Err(Error::VersionMismatch);
        }
        if room.started_at.is_some() {
            return Err(Error::GameStarted);
        }
        if room.is_full() {
            return Err(Error::RoomFull);
        }

        // 参加者はいま開いているレコードから受け取る (§2.6)。
        room.advance(self.config.tick, Instant::now());
        let user = User::new(name)?;
        let joined = Joined {
            session_id: user.session_id,
            user_id: user.id,
            room_id: room.id,
            room_info: room.info.clone(),
            tick: room.open_tick(),
        };
        self.sessions.insert(
            user.session_id,
            Session {
                room: number,
                user: user.id,
            },
        );
        tracing::info!(id = %room.id, %number, user_id = %user.id, "部屋に参加しました");
        // ユーザー ID は UUIDv7 で単調に増えるため、末尾へ足せば昇順が保たれる (§3.4)。
        room.users.push(user);
        Ok(joined)
    }

    /// ゲームを開始する (§4.5)。
    ///
    /// # Errors
    /// セッションが無効、部屋主でない、または既に開始している場合。
    pub fn start(&mut self, session_id: Uuid) -> Result<(), Error> {
        self.sweep();
        // セッションの検証を先に行い、部屋の存在に言及しない (§5.3)。
        let session = *self
            .sessions
            .get(&session_id)
            .ok_or(Error::InvalidSession)?;
        let room = self
            .by_number
            .get_mut(&session.room)
            .ok_or(Error::RoomNotFound)?;
        if room.owner_id() != Some(session.user) {
            return Err(Error::NotOwner);
        }
        if room.started_at.is_some() {
            return Err(Error::GameStarted);
        }
        room.started_at = Some(Instant::now());
        tracing::info!(id = %room.id, number = %room.number, "ゲームを開始しました");
        Ok(())
    }

    /// 同期する (§4.4)。
    ///
    /// 返せるレコードがまだ無ければ [`Sync::Wait`] を返す。呼び出し側は期限か
    /// 締め切りの通知を待って、もう一度呼ぶ。イベントは最初の 1 回だけ預ける。
    ///
    /// # Errors
    /// セッションが無効、二重同期、保持期間外の `last_tick` などの場合。
    pub fn sync(&mut self, request: SyncRequest) -> Result<Sync, Error> {
        self.sweep();
        let session = *self
            .sessions
            .get(&request.session_id)
            .ok_or(Error::InvalidSession)?;
        let tick = self.config.tick;
        let retention = self.config.record_retention;
        let room = self
            .by_number
            .get_mut(&session.room)
            .ok_or(Error::RoomNotFound)?;

        room.advance(tick, Instant::now());
        room.trim(retention);
        let is_owner = room.owner_id() == Some(session.user);

        if let Some(deposit) = request.deposit {
            if room.has_deposited(session.user) {
                return Err(Error::AlreadySynced);
            }
            if request.last_tick > room.open_tick() {
                return Err(Error::BadRequest(
                    "last_tick が未来のレコードを指しています。".to_owned(),
                ));
            }
            if room
                .oldest_tick()
                .is_some_and(|oldest| request.last_tick < oldest)
            {
                return Err(Error::SyncTooOld);
            }
            if is_owner && let Some(info) = deposit.room_info {
                room.queue_info(info);
            }
            let attach = |events: Vec<Incoming>| {
                events
                    .into_iter()
                    .map(|event| event.sent_by(session.user))
                    .collect()
            };
            room.deposit(
                session.user,
                attach(deposit.reports),
                attach(deposit.actions),
            );
            if room.everyone_arrived() {
                room.close();
                room.trim(retention);
            }
        }

        if room.records_from(request.last_tick).next().is_some() {
            return Ok(Sync::Ready(session.user));
        }
        Ok(Sync::Wait {
            deadline: room.record_deadline(tick),
            closed: room.subscribe(),
        })
    }

    /// セッションが属する部屋。
    ///
    /// # Errors
    /// セッションが無効、または部屋が消えている場合。
    pub fn room_of(&self, session_id: Uuid) -> Result<&Room, Error> {
        let session = self
            .sessions
            .get(&session_id)
            .ok_or(Error::InvalidSession)?;
        self.by_number.get(&session.room).ok_or(Error::RoomNotFound)
    }

    /// 空いている部屋番号を引く。使用中なら引き直す (§3.2)。
    fn take_number(&self) -> Result<RoomNumber, Error> {
        (0..NUMBERING_ATTEMPTS)
            .map(|_| RoomNumber::random())
            .find(|number| !self.by_number.contains_key(number))
            .ok_or(Error::RoomLimitReached)
    }
}

/// `sync` に預ける内容。
#[derive(Debug)]
pub struct SyncRequest {
    pub session_id: Uuid,
    pub last_tick: u64,
    /// 2 回目以降の呼び出しでは `None`。イベントを二重に預けないため。
    pub deposit: Option<Deposit>,
}

#[derive(Debug)]
pub struct Deposit {
    pub reports: Vec<Incoming>,
    pub actions: Vec<Incoming>,
    pub room_info: Option<Value>,
}

#[derive(Debug)]
pub enum Sync {
    /// 返せるレコードがある。値は送信者のユーザー ID。
    Ready(Uuid),
    /// まだ無い。期限か通知を待つ。
    Wait {
        deadline: Instant,
        closed: watch::Receiver<Option<u64>>,
    },
}

/// `join` の結果。部屋への借用を返さずに済むよう、必要な値だけ取り出す。
#[derive(Debug)]
pub struct Joined {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub room_id: Uuid,
    pub room_info: Value,
    pub tick: u64,
}

/// ハンドラ間で共有する登録簿。
#[derive(Debug, Clone)]
pub struct Shared(Arc<Mutex<Rooms>>);

impl Shared {
    #[must_use]
    pub fn new(config: Arc<Config>) -> Self {
        Self(Arc::new(Mutex::new(Rooms {
            config,
            by_number: HashMap::new(),
            sessions: HashMap::new(),
        })))
    }

    /// ロックを取る。
    ///
    /// 毒された場合は復帰させる。登録簿は不変条件を跨いで壊れる構造を持たず、
    /// 1 部屋の panic で以降の全リクエストを落とす方が損害が大きい。
    pub fn lock(&self) -> MutexGuard<'_, Rooms> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
