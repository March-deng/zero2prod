use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration;
use zero2prod::email_client;
use zero2prod::email_client::EmailClient;
use zero2prod::run;
use zero2prod::telemetry::{get_subsciber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subsciber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    // read config
    let config = configuration::get_config().expect("Unable to read config");

    let db_pool = PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(config.database.with_db());

    let sender_email = config
        .email_client
        .sender()
        .expect("Invalid sender email address from config");

    let email_client = EmailClient::new(config.email_client.base_url, sender_email);

    let addr = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(&addr).unwrap();

    println!("server listening on port: {}", &addr);
    run(listener, db_pool, email_client)?.await
}
