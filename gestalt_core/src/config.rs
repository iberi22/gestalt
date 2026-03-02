use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub namespace: String,
    pub database: String,
}
