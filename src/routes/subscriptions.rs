use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{
    http::StatusCode,
    web::{self},
    HttpResponse, ResponseError,
};
use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::PgPool;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;

        Ok(Self { email, name })
    }
}

#[tracing::instrument(
    name = "Adding new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;
    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a postgres connection for transaction")?;

    let subscriber_id = insert_subscriber(&mut transaction, &subscriber)
        .await
        .context("")?;

    let subscription_token = generate_subcription_token();

    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("")?;

    transaction.commit().await.context("")?;

    send_confirmation_email(&email_client, subscriber, &base_url.0, "mytoken")
        .await
        .context("")?;

    Ok(HttpResponse::Ok().finish())
}

fn generate_subcription_token() -> String {
    let mut rng = thread_rng();

    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(
    name = "saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<(Uuid), sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(transaction)
    .await
    .map_err(|e| {
        tracing::error!("Unable to execute queryL: {:?}", e);
        e
    })?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "send confirmation email to subscriber",
    skip(email_client, subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );

    let html_content = format!(
        "Welcome to our newsletter!<br /> \
    Click <a href=\"{}\">here</a> to confirm you subscription.",
        confirmation_link
    );

    let text_content = format!(
        "Welcome to our newsletter!\n Visit {} to confirm you subscription",
        confirmation_link
    );
    email_client
        .send_email(&subscriber.email, "Welcome!", &html_content, &text_content)
        .await
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for StoreTokenError {}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();

    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }

    Ok(())
}

#[tracing::instrument(
    name = "store subscription token in the database",
    skip(subscriber_id, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
    INSERT INTO subsciption_tokens (subscriber_id, subscription_token)
    VALUES ($1, $2)
        "#,
        subscriber_id,
        subscription_token,
    )
    .execute(transaction)
    .await
    .map_err(|e| StoreTokenError(e))?;

    Ok(())
}
