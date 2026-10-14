//! HTTP runtime for ModelDeck resources. Client configuration is managed only
//! by the explicit apply/restore operations in `deck::native`.

use crate::database::Database;
use crate::proxy::types::{ProxyServerInfo, ProxyStatus};
use axum::{
    extract::DefaultBodyLimit,
    routing::{any, get},
    Json, Router,
};
use std::{net::IpAddr, sync::Arc, time::Instant};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

struct RunningGateway {
    info: ProxyServerInfo,
    started: Instant,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct GatewayService {
    db: Arc<Database>,
    running: Arc<Mutex<Option<RunningGateway>>>,
}

impl GatewayService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            running: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<ProxyServerInfo, String> {
        let mut running = self.running.lock().await;
        if let Some(server) = running.as_ref().filter(|server| !server.task.is_finished()) {
            return Ok(server.info.clone());
        }
        *running = None;
        let mut config = self
            .db
            .get_global_proxy_config()
            .await
            .map_err(|error| error.to_string())?;
        let address: IpAddr = config
            .listen_address
            .parse()
            .map_err(|_| "Enter an IPv4 or IPv6 gateway listen address")?;
        let listener = tokio::net::TcpListener::bind((address, config.listen_port))
            .await
            .map_err(|error| format!("Cannot start the ModelDeck gateway: {error}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        crate::deck::publish_origin(&config.listen_address, port)?;
        crate::model_clients::publish_gateway_origin(&config.listen_address, port)?;
        crate::proxy::http_client::set_proxy_port(port);
        config.listen_port = port;
        config.proxy_enabled = true;
        self.db
            .update_global_proxy_config(config.clone())
            .await
            .map_err(|error| error.to_string())?;
        let info = ProxyServerInfo {
            address: config.listen_address,
            port,
            started_at: chrono::Utc::now().to_rfc3339(),
        };
        let router = Self::router();
        let (shutdown, receive_shutdown) = oneshot::channel();
        let task = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = receive_shutdown.await;
                })
                .await
            {
                log::error!("ModelDeck gateway stopped: {error}");
            }
        });
        log::info!(
            "ModelDeck gateway listening on {}:{}",
            info.address,
            info.port
        );
        *running = Some(RunningGateway {
            info: info.clone(),
            started: Instant::now(),
            shutdown,
            task,
        });
        Ok(info)
    }

    pub fn router() -> Router {
        Router::new()
            .route("/deck/:endpoint/*path", any(crate::deck::gateway::handle))
            .route(
                "/deck-clients/:client/*path",
                any(crate::deck::gateway::handle_harness),
            )
            // Read compatibility for connections saved before the resource graph.
            .route(
                "/model-clients/:client/*path",
                any(crate::model_clients::gateway::handle),
            )
            .route(
                "/muse-code/models",
                get(crate::model_clients::gateway::muse_models),
            )
            .route(
                "/v1/muse-code/models",
                get(crate::model_clients::gateway::muse_models),
            )
            .route(
                "/health",
                get(|| async {
                    Json(serde_json::json!({
                        "status": "ok", "service": "ModelDeck",
                        "version": env!("CARGO_PKG_VERSION")
                    }))
                }),
            )
            .layer(DefaultBodyLimit::max(200 * 1024 * 1024))
    }

    async fn shutdown(&self, persist_disabled: bool) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if persist_disabled {
            let mut config = self
                .db
                .get_global_proxy_config()
                .await
                .map_err(|error| error.to_string())?;
            config.proxy_enabled = false;
            self.db
                .update_global_proxy_config(config)
                .await
                .map_err(|error| error.to_string())?;
        }
        if let Some(mut server) = running.take() {
            let _ = server.shutdown.send(());
            if tokio::time::timeout(std::time::Duration::from_secs(5), &mut server.task)
                .await
                .is_err()
            {
                server.task.abort();
                let _ = server.task.await;
            }
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.shutdown(true).await
    }

    pub async fn stop_for_exit(&self) -> Result<(), String> {
        self.shutdown(false).await
    }

    pub async fn is_running(&self) -> bool {
        self.running.lock().await.is_some()
    }

    pub async fn get_status(&self) -> Result<ProxyStatus, String> {
        Ok(match self.running.lock().await.as_ref() {
            Some(server) => ProxyStatus {
                running: true,
                address: server.info.address.clone(),
                port: server.info.port,
                uptime_seconds: server.started.elapsed().as_secs(),
                ..Default::default()
            },
            None => ProxyStatus::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn only_modeldeck_routes_are_exposed() {
        for path in [
            "/v1/messages",
            "/v1/responses",
            "/gemini/v1beta/models",
            "/openclaw/v1/chat/completions",
        ] {
            let result = GatewayService::router()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(result.status(), axum::http::StatusCode::NOT_FOUND, "{path}");
        }
        let health = GatewayService::router()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), axum::http::StatusCode::OK);
    }
}
