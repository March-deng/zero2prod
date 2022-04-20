use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use wiremock::MockServer;
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
    pub email_server: MockServer,
    pub port: u16,
}

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Unable to execute request")
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.address))
            .json(&body)
            .send()
            .await
            .expect("Unable to execute request")
    }

    pub fn get_confirmation_link(&self, request: &wiremock::Request) -> ConfirmationLinks {
        let body = serde_json::from_slice::<serde_json::Value>(&request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();

            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();

            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(&body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(&body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }
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

    let email_server = MockServer::start().await;

    let config = {
        let mut c = get_config().expect("Unable to read config");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
        c
    };

    let application = Application::build(config.clone())
        .await
        .expect("Unable to build application");

    let port = application.port();
    let address = format!("http://127.0.0.1:{}", port);

    configure_database(&config.database).await;

    let db_pool = zero2prod::startup::get_connection_pool(&config.database);

    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        address,
        db_pool,
        email_server,
        port,
    }
}
