#[tokio::main]
async fn main() {
    env_logger::init();
    squid::App::new().run().await;
}
