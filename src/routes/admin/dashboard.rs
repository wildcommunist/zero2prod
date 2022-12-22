use crate::session_state::TypedSession;
use actix_session::Session;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::HttpResponse;
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

//region Structs & implementations
//endregion

//region HTTP handlers
pub async fn admin_dashboard(
    session: TypedSession,
    pool: Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // When using Session::get we must specify what type we want to deserialize the session state entry into -
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(e500)?
    } else {
        todo!()
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
<p>Welcome {username}!</p>
</body>
</html>"#
        )))
}
//endregion

//region Helper functions
#[tracing::instrument(name = "Get username", skip(pool))]
async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
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
