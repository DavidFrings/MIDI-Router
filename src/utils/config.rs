use crate::router::mapping_config::MappingConfig;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) dev: bool,
    #[serde(rename = "router")]
    pub(crate) router: RouterConfig,
    #[serde(rename = "maps")]
    pub(crate) maps: MappingConfig,
    #[serde(rename = "api")]
    pub(crate) api: ApiConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RouterConfig {
    pub(crate) controller_name: String,
    pub(crate) software_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ApiConfig {
    pub(crate) enabled: bool,
    pub(crate) bind_address: String,
    pub(crate) port: u16,
}

impl Config {
    pub fn new() -> Result<Self> {
        if let Ok(data) = fs::read_to_string("config.toml") {
            let config: Config = toml::from_str(&data)?;

            Ok(config)
        } else {
            Err(anyhow!("Could not find config.toml"))
        }
    }
}
