// Stopped at 3.7 (page 224)

use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let configuration = get_configuration().expect("Failed to read configuration file");
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let connection_pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(
            configuration.database.pool_timeout_seconds,
        ))
        .connect_lazy_with(configuration.database.with_db());
    run(TcpListener::bind(address)?, connection_pool)?.await
}
