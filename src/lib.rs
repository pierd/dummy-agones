mod agones_game_server_state;
mod agones_server;
mod yaml_server;

pub mod sdk {
    tonic::include_proto!("agones.dev.sdk");
}

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

pub use agones_server::AgonesServer;
pub use agones_server::DEFAULT_AGONES_PORT;
use hyper::server::conn::AddrIncoming;

impl yaml_server::Store<sdk::GameServer> for AgonesServer {
    fn get(&self) -> sdk::GameServer {
        self.game_server()
    }

    fn set(&self, value: sdk::GameServer) {
        self.set_game_server(value);
    }
}

pub struct Server {
    make_service: hybrid_service::HybridMakeService<
        axum::routing::IntoMakeService<axum::Router>,
        tonic::transport::server::Routes,
    >,
}

impl Server {
    /// Start the server with the default configuration (localhost with default Agones port).
    pub async fn serve(&self) -> Result<(), hyper::Error> {
        let incoming = AddrIncoming::bind(&SocketAddr::V4(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            DEFAULT_AGONES_PORT,
        )))?;
        self.serve_with_incoming(incoming).await
    }

    /// Spawn the server in background with random port.
    pub async fn spawn_on_random_port(&self) -> Result<u16, hyper::Error> {
        let incoming =
            AddrIncoming::bind(&SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)))?;
        let port = incoming.local_addr().port();
        let make_service = self.make_service.clone();
        tokio::spawn(hyper::Server::builder(incoming).serve(make_service));
        Ok(port)
    }

    /// Start the server with the given incoming.
    pub async fn serve_with_incoming(&self, incoming: AddrIncoming) -> Result<(), hyper::Error> {
        hyper::Server::builder(incoming)
            .serve(self.make_service.clone())
            .await
    }
}

impl Default for Server {
    fn default() -> Self {
        let agones_server = AgonesServer::default();
        let make_service = hybrid_service::hybrid(
            yaml_server::create_service(agones_server.clone()),
            agones_server.into_service(),
        );
        Self { make_service }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use parking_lot::RwLock;
    use tokio_stream::StreamExt;

    use crate::agones_game_server_state::GameServerState;

    use super::*;

    struct TestServer {
        port: u16,
        sdk: agones::Sdk,
    }

    impl TestServer {
        async fn new() -> Self {
            let port = Server::default().spawn_on_random_port().await.unwrap();
            let sdk = agones::Sdk::new(port.into(), None).await.unwrap();
            Self { port, sdk }
        }

        async fn http_get(&self) -> reqwest::Response {
            let client = reqwest::Client::new();
            client
                .get(format!("http://localhost:{}", self.port))
                .send()
                .await
                .unwrap()
        }

        async fn get(&self) -> sdk::GameServer {
            let response = self.http_get().await;
            serde_yaml::from_str(&response.text().await.unwrap()).unwrap()
        }

        async fn post(&self, gs: &sdk::GameServer) -> sdk::GameServer {
            let response = reqwest::Client::new()
                .post(format!("http://localhost:{}", self.port))
                .header(reqwest::header::CONTENT_TYPE, "application/yaml")
                .body(serde_yaml::to_vec(gs).unwrap())
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
            serde_yaml::from_str(&response.text().await.unwrap()).unwrap()
        }
    }

    #[tokio::test]
    async fn should_have_default_game_server_after_start_in_http_interface() {
        // Arrange
        let test_server = TestServer::new().await;

        // Act

        // Assert
        let gs = test_server.get().await;
        assert_eq!(gs, Default::default());
    }

    #[tokio::test]
    async fn should_have_default_game_server_after_start_in_agones_sdk() {
        // Arrange
        let mut test_server = TestServer::new().await;

        // Act

        // Assert
        let gs = test_server.sdk.get_gameserver().await.unwrap();
        assert_eq!(gs, Default::default());
    }

    #[tokio::test]
    async fn sdk_ready_should_be_visible_in_http_interface() {
        // Arrange
        let mut test_server = TestServer::new().await;

        // Act
        test_server.sdk.ready().await.unwrap();

        // Assert
        let gs = test_server.get().await;
        assert_eq!(gs.status.unwrap().state, GameServerState::Ready.to_string());
    }

    #[tokio::test]
    async fn updates_via_http_should_be_visible_in_agones_sdk() {
        // Arrange
        let mut test_server = TestServer::new().await;

        // Act
        test_server
            .post(&sdk::GameServer {
                status: Some(sdk::game_server::Status {
                    state: GameServerState::Ready.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await;

        // Assert
        let gs = test_server.sdk.get_gameserver().await.unwrap();
        assert_eq!(gs.status.unwrap().state, GameServerState::Ready.to_string());
    }

    #[tokio::test]
    async fn updates_via_http_should_be_visible_in_agones_sdk_stream() {
        // Arrange
        let mut test_server = TestServer::new().await;
        let last_game_server = Arc::new(RwLock::new(None));

        // Act
        let mut stream = test_server.sdk.watch_gameserver().await.unwrap();
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
        test_server
            .post(&sdk::GameServer {
                status: Some(sdk::game_server::Status {
                    state: GameServerState::Ready.to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Assert
        let gs = last_game_server.read().clone().unwrap();
        assert_eq!(gs.status.unwrap().state, GameServerState::Ready.to_string());
    }

    #[tokio::test]
    async fn updates_via_http_should_return_updated_game_server() {
        // Arrange
        let test_server = TestServer::new().await;
        let game_server = sdk::GameServer {
            status: Some(sdk::game_server::Status {
                state: GameServerState::Ready.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Act
        let returned_game_server = test_server.post(&game_server).await;

        // Assert
        assert_eq!(game_server, returned_game_server);
    }
}
