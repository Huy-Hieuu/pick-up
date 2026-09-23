//! Bounded, per-process invalidations backed by PostgreSQL LISTEN/NOTIFY.
//! No payload queues: slow consumers only need the latest committed snapshot.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use sqlx::{
    postgres::{PgListener, PgPoolOptions},
    PgPool,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

const MAX_CONNECTIONS: usize = 1_000;
const MAX_PER_USER: usize = 5;
const CHANNEL: &str = "pickup_game_changed";

#[derive(Clone, Default)]
pub struct LobbyHub(Arc<Mutex<HubState>>);

#[derive(Default)]
struct HubState {
    rooms: HashMap<Uuid, watch::Sender<()>>,
    users: HashMap<Uuid, usize>,
    connections: usize,
}

pub struct LobbySubscription {
    hub: LobbyHub,
    game_id: Uuid,
    user_id: Uuid,
    receiver: watch::Receiver<()>,
}

impl LobbyHub {
    pub fn subscribe(&self, game_id: Uuid, user_id: Uuid) -> AppResult<LobbySubscription> {
        let mut inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if inner.connections >= MAX_CONNECTIONS
            || inner.users.get(&user_id).copied().unwrap_or(0) >= MAX_PER_USER
        {
            return Err(AppError::TooManyRequests(
                "Too many lobby connections".into(),
            ));
        }
        let receiver = inner
            .rooms
            .entry(game_id)
            .or_insert_with(|| watch::channel(()).0)
            .subscribe();
        inner.connections += 1;
        *inner.users.entry(user_id).or_default() += 1;
        Ok(LobbySubscription {
            hub: self.clone(),
            game_id,
            user_id,
            receiver,
        })
    }

    fn invalidate(&self, game_id: Uuid) {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = inner.rooms.get(&game_id) {
            sender.send_replace(());
        }
    }

    fn invalidate_all(&self) {
        let inner = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for sender in inner.rooms.values() {
            sender.send_replace(());
        }
    }

    /// Establish LISTEN before returning, so startup cannot race the first write.
    /// Uses one dedicated DB connection, never a slot in the HTTP query pool.
    pub async fn start_listener(
        &self,
        pool: &PgPool,
        cancel: CancellationToken,
    ) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let listener_pool = PgPoolOptions::new()
            .max_connections(1)
            .max_lifetime(None)
            .idle_timeout(None)
            .acquire_timeout(Duration::from_secs(5))
            .connect_lazy_with(
                (*pool.connect_options())
                    .clone()
                    .application_name("pickup-lobby-listener"),
            );
        let mut listener = PgListener::connect_with(&listener_pool).await?;
        listener.listen(CHANNEL).await?;
        let hub = self.clone();
        Ok(tokio::spawn(async move {
            let mut retry = 1;
            loop {
                let result = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => break,
                    result = listener.try_recv() => result,
                };
                match result {
                    Ok(Some(notification)) => {
                        retry = 1;
                        if let Ok(id) = Uuid::parse_str(notification.payload()) {
                            hub.invalidate(id);
                        }
                    }
                    Ok(None) => {
                        // SQLx has reconnected and re-LISTENed. Recover anything
                        // missed during disconnection from durable game state.
                        hub.invalidate_all();
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Lobby listener interrupted; retrying");
                        hub.invalidate_all();
                        tokio::select! {
                            _ = cancel.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_secs(retry)) => {},
                        }
                        retry = (retry * 2).min(30);
                    }
                }
            }
            drop(listener);
            listener_pool.close().await;
        }))
    }
}

impl LobbySubscription {
    pub async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        self.receiver.changed().await
    }
}

impl Drop for LobbySubscription {
    fn drop(&mut self) {
        let mut inner = self.hub.0.lock().unwrap_or_else(|e| e.into_inner());
        if inner
            .rooms
            .get(&self.game_id)
            .is_some_and(|s| s.receiver_count() == 1)
        {
            inner.rooms.remove(&self.game_id);
        }
        if let Some(count) = inner.users.get_mut(&self.user_id) {
            *count -= 1;
            if *count == 0 {
                inner.users.remove(&self.user_id);
            }
        }
        inner.connections -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rooms_are_isolated_and_changes_coalesce() {
        let hub = LobbyHub::default();
        let game = Uuid::new_v4();
        let mut a = hub.subscribe(game, Uuid::new_v4()).unwrap();
        let mut b = hub.subscribe(Uuid::new_v4(), Uuid::new_v4()).unwrap();
        for _ in 0..100_000 {
            hub.invalidate(game);
        }
        a.changed().await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(10), a.changed())
            .await
            .is_err());
        assert!(tokio::time::timeout(Duration::from_millis(10), b.changed())
            .await
            .is_err());
        hub.invalidate_all();
        b.changed().await.unwrap();
    }

    #[test]
    fn connection_limits_and_cleanup_are_exact() {
        let hub = LobbyHub::default();
        let game = Uuid::new_v4();
        let user = Uuid::new_v4();
        let connections: Vec<_> = (0..MAX_PER_USER)
            .map(|_| hub.subscribe(game, user).unwrap())
            .collect();
        assert!(hub.subscribe(game, user).is_err());
        drop(connections);
        let inner = hub.0.lock().unwrap();
        assert!(inner.rooms.is_empty());
        assert!(inner.users.is_empty());
        assert_eq!(inner.connections, 0);
        drop(inner);
        let connections: Vec<_> = (0..MAX_CONNECTIONS)
            .map(|_| hub.subscribe(game, Uuid::new_v4()).unwrap())
            .collect();
        assert!(hub.subscribe(game, Uuid::new_v4()).is_err());
        drop(connections);
        assert!(hub.subscribe(game, user).is_ok());
    }
}
