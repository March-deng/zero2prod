use uuid::Uuid;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::helper::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
async fn newsletter_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;

    create_unconfirmed_subscriber(&app);

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_req_body = serde_json::json!({
        "title": "Newletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });

    let response = app.post_newsletters(newsletter_req_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletter_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;

    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_req_body = serde_json::json!({
        "title": "Newletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });
    let response = app.post_newsletters(newsletter_req_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletter_returns_400_for_invalid_data() {
    let app = spawn_app().await;

    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>",
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "Newletter title"
            }),
            "missing content",
        ),
    ];

    for (body, msg) in test_cases {
        let response = app.post_newsletters(body).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The api did not fail with 400 when {}",
            msg
        );
    }
}

#[tokio::test]
async fn non_existing_user_is_rejected() {
    let app = spawn_app().await;

    let username = Uuid::new_v4().to_string();
    let password = Uuid::new_v4().to_string();

    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "Newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>",
            }
        }))
        .send()
        .await
        .expect("Unable to execute request.");

    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
    assert_eq!(401, response.status().as_u16());
}

#[tokio::test]
async fn invalid_password_is_rejected() {
    let app = spawn_app().await;
    let username = &app.test_user.username;
    let password = Uuid::new_v4().to_string();

    assert_ne!(password, app.test_user.password);

    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", app.address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
            "title": "Newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>",
            }
        }))
        .send()
        .await
        .expect("Unable to execute request.");

    assert_eq!(401, response.status().as_u16());
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });
    let response = app.post_newsletters(body).await;

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create uncofirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_req = app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_link(&email_req)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

// async fn newsletter_creation_is_idempotent() {
//     let app = spawn_app().await;
//     create_confirmed_subscriber(&app).await;

//     app.test_user.login().await;

//     Mock::given(path("/email"))
//         .and(method("POST"))
//         .respond_with(ResponseTemplate::new(200))
//         .expect(1)
//         .mount(&app.email_server)
//         .await;

//     let newletter_request_body = serde_json::json!({
//         "title": "Newsletter title",
//         "text_content": "Newletter body as plain text",
//         "html_content": "<p>Newsletter body as HTML</p>",
//         "idempotency_key": uuid::Uuid::new_v4().to_string()
//     });

//     let resp = app.post_publish_newsletter(&newletter_request_body).await;
//     assert_is_redirect_to(&resp, "/admin/newsletter");
// }

// #[tokio::test]
// async fn concurrent_form_submission_is_handled_gracefully() {
//     // Arrange
//     let app = spawn_app().await;
//     create_confirmed_subscriber(&app).await;
//     app.test_user.login(&app).await;

//     Mock::given(path("/email"))
//         .and(method("POST"))
//         // Setting a long delay to ensure that the second request
//         // arrives before the first one completes
//         .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
//         .expect(1)
//         .mount(&app.email_server)
//         .await;

//     // Act - Submit two newsletter forms concurrently
//     let newsletter_request_body = serde_json::json!({
//         "title": "Newsletter title",
//         "text_content": "Newsletter body as plain text",
//         "html_content": "<p>Newsletter body as HTML</p>",
//         "idempotency_key": uuid::Uuid::new_v4().to_string()
//     });
//     let response1 = app.post_publish_newsletter(&newsletter_request_body);
//     let response2 = app.post_publish_newsletter(&newsletter_request_body);
//     let (response1, response2) = tokio::join!(response1, response2);

//     assert_eq!(response1.status(), response2.status());
//     assert_eq!(response1.text().await.unwrap(), response2.text().await.unwrap());

//     // Mock verifies on Drop that we have sent the newsletter email **once**
// }
