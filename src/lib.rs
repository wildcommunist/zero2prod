pub mod configuration;
pub mod routes;
pub mod startup;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}
