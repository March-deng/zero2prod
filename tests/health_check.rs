use once_cell::sync::Lazy;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use zero2prod::configuration::{get_config, DBSettings};
use zero2prod::email_client::EmailClient;
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

#[tokio::test]
async fn health_check_works() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health_check", app.address))
        .send()
        .await
        .expect("Failed to execute request");

    println!("{}", resp.status());
    assert!(resp.status().is_success());
    assert_eq!(Some(0), resp.content_length());
}

#[tokio::test]
async fn subscribe_works_for_valid_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=dengcong&email=marchdeng%40email.com";
    let resp = client
        .post(&format!("{}/subscriptions", app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Unable to execute request");

    println!("{}", resp.status());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Unable to fetch saved subscription.");
    assert_eq!(saved.email, "marchdeng@email.com");
    assert_eq!(saved.name, "dengcong");
    assert_eq!(200, resp.status().as_u16());
}

#[tokio::test]
async fn subscribe_bad_req_for_broken_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let cases = vec![
        ("name=le%20guin", "missing email"),
        ("email=marchdeng%40mail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (body, err_msg) in cases {
        let resp = client
            .post(&format!("{}/subscriptions", app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Unable to execute request");

        assert_eq!(
            400,
            resp.status().as_u16(),
            "API fail with error message: {}",
            err_msg
        );
    }
}

#[tokio::test]
async fn subscribe_returns_400_when_fields_present_but_empty() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let resp = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Unable to execute request");
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not return 400 Bad Request when the payload was {}.",
            description
        );
    }
}

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub async fn configure_database(config: &DBSettings) -> PgPool {
    let db_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Unable to connect to PG");
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Unable to migrate database schema");

    db_pool
}

async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Unable to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let config = get_config().expect("Unable to read config");

    let db_pool = configure_database(&config.database).await;

    let sender_email = config
        .email_client
        .sender()
        .expect("Invalid sender email address");

    let email_client = EmailClient::new(config.email_client.base_url, sender_email);

    let server =
        zero2prod::run(listener, db_pool.clone(), email_client).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp { address, db_pool }
}
