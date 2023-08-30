use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    result::Result,
    sync::Arc,
};

use futures::{StreamExt, TryStreamExt};
use parking_lot::RwLock;
use tokio::net::{TcpListener, ToSocketAddrs};
use tonic::{async_trait, transport::server::TcpIncoming};

use crate::{
    agones_game_server_state::GameServerState,
    sdk::{self, Duration, Empty, GameServer, KeyValue},
};

pub const DEFAULT_AGONES_PORT: u16 = 9357;

#[derive(Clone)]
pub struct AgonesServer {
    stored_game_server: Arc<RwLock<GameServer>>,
    game_server_sender: tokio::sync::broadcast::Sender<GameServer>,
}

impl AgonesServer {
    pub async fn start(&self) -> Result<SocketAddr, std::io::Error> {
        self.start_with_port(DEFAULT_AGONES_PORT).await
    }

    pub async fn start_with_port(&self, port: u16) -> Result<SocketAddr, std::io::Error> {
        let localhost_with_port = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        self.start_with_addr(localhost_with_port).await
    }

    pub async fn start_with_addr(
        &self,
        addr: impl ToSocketAddrs,
    ) -> Result<SocketAddr, std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let incoming =
            TcpIncoming::from_listener(listener, false, Some(std::time::Duration::from_secs(30)))
                .unwrap();

        tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(sdk::sdk_server::SdkServer::new(self.clone()))
                .serve_with_incoming(incoming),
        );

        Ok(local_addr)
    }

    pub fn into_service(&self) -> tonic::transport::server::Routes {
        tonic::transport::Server::builder()
            .add_service(sdk::sdk_server::SdkServer::new(self.clone()))
            .into_service()
    }

    pub fn game_server(&self) -> GameServer {
        self.stored_game_server.read().clone()
    }

    pub fn set_game_server(&self, gs: GameServer) {
        tracing::debug!("Setting game server: {:?}", gs);
        self.stored_game_server.write().clone_from(&gs);
        let _ = self.game_server_sender.send(gs);
    }

    fn set_game_server_state(&self, state: GameServerState) {
        self.mutate_game_server(|gs| {
            let mut status = gs.status.take().unwrap_or_default();
            status.state = state.to_string();
            gs.status = Some(status);
        });
    }

    fn mutate_game_server<F>(&self, f: F)
    where
        F: FnOnce(&mut GameServer),
    {
        let mut gs = self.stored_game_server.write();
        f(&mut gs);
        tracing::debug!("Mutated game server to: {:?}", gs);
        let _ = self.game_server_sender.send(gs.clone());
    }
}

impl Default for AgonesServer {
    fn default() -> Self {
        let (game_server_sender, _) = tokio::sync::broadcast::channel(1);

        Self {
            stored_game_server: Default::default(),
            game_server_sender,
        }
    }
}

#[async_trait]
impl sdk::sdk_server::Sdk for AgonesServer {
    /// Call when the GameServer is ready
    async fn ready(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        tracing::info!("Got ready call");
        self.set_game_server_state(GameServerState::Ready);
        Ok(tonic::Response::new(Empty {}))
    }

    /// Call to self Allocation the GameServer
    async fn allocate(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        tracing::info!("Got allocate call");
        self.set_game_server_state(GameServerState::Allocated);
        Ok(tonic::Response::new(Empty {}))
    }

    /// Call when the GameServer is shutting down
    async fn shutdown(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        tracing::info!("Got shutdown call");
        self.set_game_server_state(GameServerState::Shutdown);
        Ok(tonic::Response::new(Empty {}))
    }

    /// Send a Empty every d Duration to declare that this GameSever is healthy
    async fn health(
        &self,
        request: tonic::Request<tonic::Streaming<Empty>>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        tracing::info!("Got health call");
        let mut stream = request.into_inner();
        tokio::spawn(async move {
            loop {
                match stream.message().await {
                    Ok(None) => {
                        tracing::info!("Got health stream end");
                        break;
                    }
                    Ok(_) => {
                        tracing::info!("Got health message");
                    }
                    Err(err) => {
                        tracing::error!("Got health stream error: {:?}", err);
                        break;
                    }
                }
            }
        });

        Ok(tonic::Response::new(Empty {}))
    }

    /// Retrieve the current GameServer data
    async fn get_game_server(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<GameServer>, tonic::Status> {
        tracing::info!("Got get_game_server call");
        Ok(tonic::Response::new(self.game_server()))
    }

    /// Server streaming response type for the WatchGameServer method.
    type WatchGameServerStream = Pin<
        Box<dyn futures_core::Stream<Item = Result<GameServer, tonic::Status>> + Send + 'static>,
    >;

    /// Send GameServer details whenever the GameServer is updated
    async fn watch_game_server(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Self::WatchGameServerStream>, tonic::Status> {
        tracing::info!("Got watch_game_server call");
        let rx = self.game_server_sender.subscribe();
        let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
            .map_err(|e| tonic::Status::internal(e.to_string()));

        // prepend current game server to notify immediately
        let gs = self.game_server();
        let stream = tokio_stream::once(Ok(gs)).chain(stream);

        Ok(tonic::Response::new(Box::pin(stream)))
    }

    /// Apply a Label to the backing GameServer metadata
    async fn set_label(
        &self,
        request: tonic::Request<KeyValue>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        let KeyValue { key, value } = request.into_inner();
        tracing::info!("Got set_label call k={:?} v={:?}", key, value);
        self.mutate_game_server(|gs| {
            let mut meta = gs.object_meta.take().unwrap_or_default();
            meta.labels.insert(key, value);
            gs.object_meta = Some(meta);
        });
        Ok(tonic::Response::new(Empty {}))
    }

    /// Apply a Annotation to the backing GameServer metadata
    async fn set_annotation(
        &self,
        request: tonic::Request<KeyValue>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        let KeyValue { key, value } = request.into_inner();
        tracing::info!("Got set_annotation call k={:?} v={:?}", key, value);
        self.mutate_game_server(move |gs| {
            let mut meta = gs.object_meta.take().unwrap_or_default();
            meta.annotations.insert(key, value);
            gs.object_meta = Some(meta);
        });
        Ok(tonic::Response::new(Empty {}))
    }

    /// Marks the GameServer as the Reserved state for Duration
    async fn reserve(
        &self,
        request: tonic::Request<Duration>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        tracing::info!("Got reserve call duration={:?}", request.into_inner());
        self.set_game_server_state(GameServerState::Reserved);
        // FIXME: use duration maybe?
        Ok(tonic::Response::new(Empty {}))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    use fake::{Fake, StringFaker};

    fn fake_string() -> String {
        StringFaker::with((b'a'..=b'z').collect(), 4..=6).fake()
    }

    async fn server_and_sdk() -> (AgonesServer, agones::Sdk) {
        // use random port
        let server = AgonesServer::default();
        let addr = server.start_with_port(0).await.unwrap();
        let sdk = agones::Sdk::new(addr.port().into(), None).await.unwrap();
        (server, sdk)
    }

    #[tokio::test]
    async fn call_to_ready_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;

        // Act
        sdk.ready().await.unwrap();

        // Assert
        assert_eq!(
            server.game_server().status.unwrap().state,
            GameServerState::Ready.to_string()
        );
    }

    #[tokio::test]
    async fn call_to_allocate_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;

        // Act
        sdk.allocate().await.unwrap();

        // Assert
        assert_eq!(
            server.game_server().status.unwrap().state,
            GameServerState::Allocated.to_string()
        );
    }

    #[tokio::test]
    async fn call_to_shutdown_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;

        // Act
        sdk.shutdown().await.unwrap();

        // Assert
        assert_eq!(
            server.game_server().status.unwrap().state,
            GameServerState::Shutdown.to_string()
        );
    }

    #[tokio::test]
    async fn call_to_get_game_server_returns_default() {
        // Arrange
        let (_server, mut sdk) = server_and_sdk().await;

        // Act
        let game_server = sdk.get_gameserver().await.unwrap();

        // Assert
        assert_eq!(game_server, Default::default());
    }

    #[tokio::test]
    async fn call_to_get_game_server_returns_current_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;
        let game_server = GameServer {
            object_meta: Some(sdk::game_server::ObjectMeta {
                name: fake_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_ne!(game_server, Default::default()); // sanity check
        server.set_game_server(game_server.clone());

        // Act
        let retrieved_game_server = sdk.get_gameserver().await.unwrap();

        // Assert
        assert_eq!(
            retrieved_game_server.object_meta.unwrap().name,
            game_server.object_meta.unwrap().name
        );
    }

    #[tokio::test]
    async fn call_to_watch_game_server_returns_current_game_server() {
        // Arrange
        let (_server, mut sdk) = server_and_sdk().await;

        // Act
        let mut stream = sdk.watch_gameserver().await.unwrap();
        let game_server =
            tokio::time::timeout(std::time::Duration::from_millis(100), stream.message())
                .await
                .unwrap()
                .unwrap()
                .unwrap();

        // Assert
        assert_eq!(game_server, Default::default());
    }

    #[tokio::test]
    async fn call_to_watch_game_server_returns_updated_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;
        let game_server = GameServer {
            object_meta: Some(sdk::game_server::ObjectMeta {
                name: fake_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_ne!(game_server, Default::default()); // sanity check

        let last_game_server = Arc::new(RwLock::new(None));
        let mut stream = sdk.watch_gameserver().await.unwrap();
        tokio::spawn({
            let last_game_server = last_game_server.clone();
            async move {
                while let Some(gs) = stream.next().await {
                    if let Ok(gs) = gs {
                        *last_game_server.write() = Some(gs);
                    }
                }
            }
        });

        // Act
        server.set_game_server(game_server.clone());
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert
        assert_eq!(
            last_game_server
                .read()
                .as_ref()
                .unwrap()
                .object_meta
                .as_ref()
                .unwrap()
                .name,
            game_server.object_meta.unwrap().name
        );
    }

    #[tokio::test]
    async fn call_to_set_label_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;
        let key = fake_string();
        let value = fake_string();

        // Act
        sdk.set_label(&key, &value).await.unwrap();

        // Assert
        assert_eq!(
            server.game_server().object_meta.unwrap().labels.get(&key),
            Some(&value)
        );
    }

    #[tokio::test]
    async fn call_to_set_annotation_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;
        let key = fake_string();
        let value = fake_string();

        // Act
        sdk.set_annotation(&key, &value).await.unwrap();

        // Assert
        assert_eq!(
            server
                .game_server()
                .object_meta
                .unwrap()
                .annotations
                .get(&key),
            Some(&value)
        );
    }

    #[tokio::test]
    async fn call_to_reserve_updates_game_server() {
        // Arrange
        let (server, mut sdk) = server_and_sdk().await;
        let seconds = (0..1000).fake();
        let duration = std::time::Duration::from_secs(seconds);

        // Act
        sdk.reserve(duration.clone()).await.unwrap();

        // Assert
        assert_eq!(
            server.game_server().status.unwrap().state,
            GameServerState::Reserved.to_string()
        );
    }
}
