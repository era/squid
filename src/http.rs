use tokio::task::JoinHandle;
use tower_http::services::ServeDir;

pub fn serve(port: u16, folder: &str) -> JoinHandle<()> {
    let service = ServeDir::new(folder);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port.clone()));

    tokio::task::spawn(async move {
        hyper::Server::bind(&addr)
            .serve(tower::make::Shared::new(service))
            .await
            .expect("server error")
    })
}
