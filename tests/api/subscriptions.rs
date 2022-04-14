use crate::helper::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};
#[tokio::test]
async fn subscribe_works_for_valid_data() {
    let app = spawn_app().await;

    let body = "name=dengcong&email=marchdeng%40email.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let resp = app.post_subscriptions(body.into()).await;

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

    let cases = vec![
        ("name=le%20guin", "missing email"),
        ("email=marchdeng%40mail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (body, err_msg) in cases {
        let resp = app.post_subscriptions(body.into()).await;

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
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let resp = app.post_subscriptions(body.into()).await;
        assert_eq!(
            400,
            resp.status().as_u16(),
            "The API did not return 400 Bad Request when the payload was {}.",
            description
        );
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;
}

async fn sunscribe_sends_a_confirmation_email_with_link() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_link(email_request);
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

#[tokio::test]
async fn subscribe_persist_the_new_subscriber() {
    let app = spawn_app().await;

    let body = "name=dengcong&email=marchdeng%40email.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let resp = app.post_subscriptions(body.into()).await;

    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.db_pool)
        .await
        .expect("Unable to fetch saved subscription.");
    assert_eq!(saved.email, "marchdeng@email.com");
    assert_eq!(saved.name, "dengcong");
    assert_eq!(saved.status, "pending_confirmation");
    assert_eq!(200, resp.status().as_u16());
}
