use crate::routes::{health_check, subscribe};
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    /*
    // Wrap the connection in a smart pointer. se page 92 of the guide that explains really well
    // why we need to wrap it. In short: PgConnection is non cloneable bu App::new() creates a new
    // instance for each cpu core
     */
    let connection = web::Data::new(db_pool);
    let server = HttpServer::new(move || {
        App::new()
            // Middleware
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(connection.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
