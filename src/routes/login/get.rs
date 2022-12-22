use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use serde::Deserialize;

//region Structs & Implementations
#[derive(Deserialize)]
pub struct QueryParams {
    error: Option<String>,
}
//endregion

//region HTTP handlers
pub async fn login_form(query: actix_web::web::Query<QueryParams>) -> HttpResponse {
    let error_html = match query.0.error {
        None => "".into(),
        Some(e) => format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&e)),
    };
    HttpResponse::Ok()
        .content_type(ContentType::html())
        //.body(include_str!("login.html"))
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
<input
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
//endregion
