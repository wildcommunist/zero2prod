use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::HttpResponse;
use anyhow::Context;
use reqwest::header::LOCATION;
use sqlx::PgPool;
use uuid::Uuid;

//region Structs & implementations
//endregion

//region HTTP handlers
pub async fn admin_dashboard(
    session: TypedSession,
    pool: Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // When using Session::get we must specify what type we want to deserialize the session state entry into -
    let (username, user_id) = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        (get_username(user_id, &pool).await.map_err(e500)?, user_id)
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
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
<p>Available actions:</p>
<ol>
<li><a href="/admin/password">Change password</a></li>
<li>
<form name="logoutForm" action="/admin/logout" method="post">
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
