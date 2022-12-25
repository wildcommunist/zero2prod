use crate::authentication::UserId;
use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use tera::Tera;
use uuid::Uuid;

//region Structs & implementations
//endregion

//region HTTP handlers
pub async fn admin_dashboard(
    user_id: web::ReqData<UserId>,
    pool: Data<PgPool>,
    tera: Data<Tera>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    let username = get_username(*user_id, &pool).await.map_err(e500)?;
    let mut context = tera::Context::new();
    context.insert("username", &username);
    context.insert("user_id", &user_id.to_string());
    Ok(HttpResponse::Ok().content_type(ContentType::html()).body(
        tera.render("admin/dashboard.html", &context)
            .map_err(e500)?,
    ))
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
