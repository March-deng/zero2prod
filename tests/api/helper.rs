use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{get_config, DBSettings};
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subsciber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subsciber("test".into(), "debug".into(), std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subsciber("test".into(), "debug".into(), std::io::sink);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub async fn configure_database(config: &DBSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("unable to connect to postgres");
    let query = format!(r#"CREATE DATABASE "{}";"#, config.database_name);
    connection
        .execute(query.as_str())
        .await
        .expect("Unable to create database");

    let db_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Unable to connect to postgreq");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("unable to migrate to the database");

    db_pool
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let config = {
        let mut c = get_config().expect("Unable to read config");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };

    let application = Application::build(config.clone())
        .await
        .expect("Unable to build application");

    let address = format!("http://127.0.0.1:{}", application.port());

    configure_database(&config.database).await;

    let db_pool = zero2prod::startup::get_connection_pool(&config.database);

    let _ = tokio::spawn(application.run_until_stopped());

    TestApp { address, db_pool }
}
