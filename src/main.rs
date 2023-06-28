mod app;
mod config;
mod http;
mod io;
mod md;
mod template;
mod tinylang;
mod watch;

#[tokio::main]
async fn main() {
    env_logger::init();
    app::App::new().run().await;
}
