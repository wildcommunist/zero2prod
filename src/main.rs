// Stopped at 3.7 (page 327)

use zero2prod::configuration::get_settings;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "debug".into(), std::io::stdout);
    init_subscriber(subscriber);
    let config = get_settings().expect("Failed to read configuration");
    let app = Application::build(config)
        .await
        .expect("Failed to create application");
    app.run_until_stopped().await?;
    Ok(())
}
