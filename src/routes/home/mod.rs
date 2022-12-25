use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::web::Data;
use actix_web::HttpResponse;
use tera::{Context, Tera};

pub async fn home(tera: Data<Tera>) -> Result<HttpResponse, actix_web::Error> {
    let context = Context::new();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(tera.render("test.html", &context).map_err(e500)?))
}
