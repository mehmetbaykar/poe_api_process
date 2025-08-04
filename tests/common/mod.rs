use dotenvy::dotenv;
use std::env;

pub fn setup() {
    // Load environment variables
    dotenv().ok();
}

#[allow(dead_code)]
pub fn get_access_key() -> String {
    env::var("POE_ACCESS_KEY")
        .expect("POE_ACCESS_KEY must be set in .env file")
}