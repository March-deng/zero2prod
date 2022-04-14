use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct ConfirmParameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    pool: web::Data<PgPool>,
    parameters: web::Query<ConfirmParameters>,
) -> HttpResponse {
    let id = match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match id {
        None => return HttpResponse::Unauthorized().finish(),
        Some(subcriber_id) => {
            if confirm_subscriber(&pool, subcriber_id).await.is_err() {
                return HttpResponse::InternalServerError().finish();
            }
        }
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "Mark subcriber as confirmed", skip(subcriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subcriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subcriber_id,
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Unable to execute update status query: {:?}", e);
        e
    })?;
    Ok(())
}

#[tracing::instrument(name = "Get subcriber_id from token", skip(token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let record = sqlx::query!(
        "SELECT subscriber_id FROM subsciption_tokens WHERE subscription_token = $1",
        token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Unable to execute query: {:?}", e);
        e
    })?;

    Ok(record.map(|r| r.subscriber_id))
}
