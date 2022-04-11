use crate::helper::spawn_app;
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
