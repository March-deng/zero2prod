use crate::helper::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmation_without_token_rejected_with_400() {
    let app = spawn_app().await;

    let resp = reqwest::get(format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(resp.status().as_u16(), 400);
}

#[tokio::test]
async fn subcribe_and_confirm_return_200() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40@gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;
    app.post_subscriptions(body.into()).await;

    let email_req = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_link(email_req);

    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}

async fn clicking_on_the_confirmation_link_confirms_subscriber() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40@gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;
    app.post_subscriptions(body.into()).await;

    let email_req = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_link(email_req);

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Unable to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "confirmed");
}
