use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct QueryParam {
    error: String,
    tag: String,
}

pub async fn login_form(query: Option<web::Query<QueryParam>>) -> HttpResponse {
    let error_html = match query {
        None => "".into(),
        Some(err_msg) => format!(
            "<p><i>{}</i></p>",
            htmlescape::encode_minimal(&err_msg.error)
        ),
    };

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
        
        “ <input
        type="password"
        placeholder="Enter Password"
        name="password"
    >
</label>
<button type="submit">Login</button>
</form>
</body>
</html>"#,
        ))
}
