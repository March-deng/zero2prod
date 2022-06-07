use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subsciber, init_subscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subsciber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    // read config
    let config = configuration::get_config().expect("Unable to read config");

    let application = Application::build(config).await?;

    application.run_until_stopped().await?;
    Ok(())
}
