use actix_web::body::to_bytes;
use actix_web::HttpResponse;
use reqwest::StatusCode;
use sqlx::postgres::PgHasArrayType;
use sqlx::PgPool;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::key::IdempotencyKey;

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
        SELECT response_status_code as "response_status_code!", response_headers as "response_headers!: Vec<HeaderPairRecord>", response_body as "response_body!" FROM idempotency WHERE user_id = $1 AND idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(pool).await?;

    if let Some(r) = saved_response {
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        let mut resp = HttpResponse::build(status_code);

        for HeaderPairRecord { name, value } in r.response_headers {
            resp.append_header((name, value));
        }
        Ok(Some(resp.body(r.response_body)))
    } else {
        Ok(None)
    }
}

pub async fn save_response(
    mut transaction: Transaction<'static, Postgres>,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (header, body) = http_response.into_parts();

    let body = to_bytes(body).await.map_err(|e| anyhow::anyhow!("{}", e))?;

    let status_code = header.status().as_u16() as i16;

    let headers = {
        let mut h = Vec::with_capacity(header.headers().len());

        for (name, value) in header.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();

            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    sqlx::query_unchecked!(
        r#"UPDATE idempotency SET response_status_code = $3, response_headers = $4, response_body = $5 WHERE user_id = $1 AND idempotency_key = $2"#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref()
    ).execute(&mut transaction).await?;

    transaction.commit().await?;

    let http_response = header.set_body(body).map_into_boxed_body();

    Ok(http_response)
}

#[allow(clippy::large_enum_variant)]
pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>),
    ReturnSavedResponse(HttpResponse),
}

pub async fn try_processing(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    let n_inserted_rows = sqlx::query!(r#"
    INSERT INTO idempotency (user_id, idempotency_key, created_at) VALUES ($1, $2, now()) ON CONFLICT DO NOTHING"#,
    user_id, idempotency_key.as_ref())
    .execute(&mut transaction).await?.rows_affected();

    if n_inserted_rows > 0 {
        Ok(NextAction::StartProcessing(transaction))
    } else {
        let saved_resp = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("We expected a saved resp, we didn't find it"))?;

        Ok(NextAction::ReturnSavedResponse(saved_resp))
    }
}
