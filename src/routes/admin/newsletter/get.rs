use crate::authentication::UserId;
use crate::routes::admin::dashboard::get_username;
use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::IncomingFlashMessages;
use sqlx::PgPool;
use std::fmt::Write;

//region HTTP handlers
pub async fn newsletter_form(
    user_id: web::ReqData<UserId>,
    pool: Data<PgPool>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &pool).await.map_err(e500)?;
    let idempotency_key = uuid::Uuid::new_v4();

    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
<title>Compose newsletter</title>
</head>
<body>
{msg_html}
<h3>{username}, compose newsletter issue.</h3>
<form name="sendNewsletterForm" action="/admin/newsletter" method="post">
<input type="text" name="idempotency_key" value="{idempotency_key}" />
<label>Issue title
<input
type="text"
placeholder="Awesome issue title"
name="title"
></label><br />
<label>HTML
<textarea
placeholder="Rich, HTML content"
name="html"
></textarea></label><br />
<label>Plain
<textarea
placeholder="Plain and boring text"
name="plain"
></textarea></label><br />
<input type="submit" value="Post It!">
</form>
<p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#
        )))
}
//endregion
