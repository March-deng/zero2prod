use std::net::TcpListener;

use crate::configuration::{DBSettings, Settings};
use crate::routes::confirm;
use crate::{
    email_client::EmailClient, routes::health_check, routes::home, routes::login,
    routes::login_form, routes::publish_newsletter, routes::subscribe,
};
use actix_web::{dev::Server, web, App, HttpServer};
use secrecy::Secret;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, std::io::Error> {
        let db_pool = PgPoolOptions::new()
            .connect_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(config.database.with_db());

        let sender_email = config
            .email_client
            .sender()
            .expect("Invalid sender email address from config");
        let timeout = config.email_client.timeout();
        let email_client = EmailClient::new(
            config.email_client.base_url,
            sender_email,
            config.email_client.authorization_token,
            timeout,
        );

        let address = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();

        let server = run(
            listener,
            db_pool,
            email_client,
            config.application.base_url,
            config.application.hmac_secret,
        )?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

pub fn run(
    listener: TcpListener,
    db: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: Secret<String>,
) -> Result<Server, std::io::Error> {
    let db = web::Data::new(db);
    let email_client = web::Data::new(email_client);
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/login", web::post().to(login))
            .route("/login", web::get().to(login_form))
            .route("/", web::get().to(home))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(db.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(web::Data::new(HmacSecret(hmac_secret.clone())))
        // .app_data()
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub fn get_connection_pool(config: &DBSettings) -> PgPool {
    PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(config.with_db())
}
