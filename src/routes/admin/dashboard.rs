use crate::authentication::UserId;
use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

//region Structs & implementations
//endregion

//region HTTP handlers
pub async fn admin_dashboard(
    user_id: web::ReqData<UserId>,
    pool: Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &pool).await.map_err(e500)?;
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta http-equiv="content-type" content="text/html; charset=utf-8">
<title>Admin dashboard</title>
</head>
<body>
<p>Welcome {username} [{user_id}]!</p>
<h3>Actions</h3>
<ol>
<li><a href="/admin/newsletter">Send a newsletter issue</a></li>
</ol>
<p>Settings:</p>
<ol>
<li><a href="/admin/password">Change password</a></li>
<li>
<form name="logoutForm" action="/logout" method="post">
<input type="submit" value="Logout">
</form>
</li>
</ol>
</body>
</html>"#
        )))
}
//endregion

//region Helper functions
#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1"#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform username fetch query")?;
    Ok(row.username)
}
//endregion