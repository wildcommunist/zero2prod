// Stopped at 3.7 (page 87)

use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration file");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    run(TcpListener::bind(address)?)?.await
}
