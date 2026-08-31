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

use uuid::Uuid;

use crate::{
    config::Config,
    error::Error,
    room::{Room, RoomNumber, User},
};

/// セッションが指す先 (§3.6)。
#[derive(Debug, Clone, Copy)]
#[expect(dead_code, reason = "sync と start の実装で読む")]
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

    /// 空いている部屋番号を引く。使用中なら引き直す (§3.2)。
    fn take_number(&self) -> Result<RoomNumber, Error> {
        (0..NUMBERING_ATTEMPTS)
            .map(|_| RoomNumber::random())
            .find(|number| !self.by_number.contains_key(number))
            .ok_or(Error::RoomLimitReached)
    }
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
