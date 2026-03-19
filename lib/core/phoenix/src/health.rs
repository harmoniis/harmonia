use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ractor::ActorRef;
use tokio::net::TcpListener;

use crate::msg::{DaemonMode, HealthSnapshot, SupervisorMsg};

pub async fn serve(port: u16, supervisor: ActorRef<SupervisorMsg>) {
    // Bind localhost only — never expose health to the network
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => {
            eprintln!("[INFO] [phoenix] Health server listening on {addr}");
            l
        }
        Err(e) => {
            eprintln!("[ERROR] [phoenix] Failed to bind health server on {addr}: {e}");
            return;
        }
    };

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(_) => continue,
        };
        let sup = supervisor.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = service_fn(move |req| {
                let sup = sup.clone();
                async move { handle_request(req, &sup).await }
            });
            let _ = http1::Builder::new().serve_connection(io, svc).await;
        });
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    supervisor: &ActorRef<SupervisorMsg>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    match req.uri().path() {
        "/health" => {
            let snapshot = query_health(supervisor).await;
            match snapshot {
                Some(s) => {
                    let body = serde_json::to_string_pretty(&s).unwrap_or_default();
                    Ok(Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Full::new(Bytes::from(body)))
                        .unwrap())
                }
                None => Ok(Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Full::new(Bytes::from("supervisor unavailable")))
                    .unwrap()),
            }
        }
        "/health/ready" => {
            let snapshot = query_health(supervisor).await;
            match snapshot {
                Some(s) if matches!(s.mode, DaemonMode::Full) => Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(Full::new(Bytes::from("ready")))
                    .unwrap()),
                _ => Ok(Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Full::new(Bytes::from("not ready")))
                    .unwrap()),
            }
        }
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("not found")))
            .unwrap()),
    }
}

async fn query_health(supervisor: &ActorRef<SupervisorMsg>) -> Option<HealthSnapshot> {
    ractor::call_t!(supervisor, SupervisorMsg::GetHealth, 15000).ok()
}
