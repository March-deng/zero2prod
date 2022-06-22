use crate::authentication::UserID;
use crate::idempotency::{save_response, try_processing, IdempotencyKey, NextAction};
use crate::utils::{e400, e500, see_other};
use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newletter issue",
    skip_all,
    fields(user_id=%&*user_id)
)]
pub async fn publish_newsletter(
    form: web::Json<FormData>,
    pool: web::Data<PgPool>,
    user_id: ReqData<UserID>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    let mut tx = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_resp) => {
            success_message().send();
            return Ok(saved_resp);
        }
    };

    let issue_id = insert_newsletter_issue(&mut tx, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;

    enqueue_delievery_tasks(&mut tx, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;

    let resp = see_other("/admin/newsletter");

    let resp = save_response(tx, &idempotency_key, *user_id, resp)
        .await
        .map_err(e500)?;

    success_message().send();
    Ok(resp)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issus has been accepted - emails will go out shortly.")
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    struct Row {
        email: String,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT email FROM subscriptions WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;

    let confirmed_subscribers = rows
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let issue_id = Uuid::new_v4();

    sqlx::query!(r#"INSERT INTO newsletter_issues (newsletter_issue_id, title, text_content, html_content, published_at) VALUES ($1, $2, $3, $4, now())"#,
    issue_id,
    title,
    text_content,
    html_content,
    ).execute(transaction).await?;

    Ok(issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delievery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(r#"INSERT INTO issue_delivery_queue (newsletter_issue_id, subscriber_email) SELECT $1, email FROM subscriptions WHERE status = 'confirmed'"#, issue_id).execute(transaction).await?;
    Ok(())
}
