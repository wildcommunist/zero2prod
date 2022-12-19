// Stopped at 3.7 (page 381)

use zero2prod::configuration::{get_settings, Settings};
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let config = get_settings().expect("Failed to read configuration");
    tracing::info!(
        "Starting application in `{}` mode",
        Settings::current_environment().as_str()
    );
    let app = Application::build(config)
        .await
        .expect("Failed to create application");
    app.run_until_stopped().await?;
    Ok(())
}
