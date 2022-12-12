use actix_web::{web, HttpResponse, Responder};
use sqlx::types::chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

pub async fn subscribe(form: web::Form<FormData>, pool: web::Data<PgPool>) -> impl Responder {
    let request_id = Uuid::new_v4();
    let request_span = tracing::info_span!(
        "Adding a new subscriber",
        %request_id,
        subscriber_email = %form.email,
        subscriber_name = %form.name
    );
    tracing::info!(
        "[{}] Adding '{}' '{}' as a new subscriber",
        request_id,
        form.name,
        form.email
    );
    match sqlx::query!(
        r#"
    INSERT INTO subscriptions (id,email,name,subscribed_at)
    VALUES ($1, $2, $3, $4)
    "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => {
            tracing::info!("[{}] New subscriber has been added", request_id,);
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            tracing::error!("[{}] Failed to save new record: {:?}", request_id, e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
