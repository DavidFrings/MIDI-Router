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
    #[serde(default, deserialize_with = "string_or_int")]
    pub(crate) pin: String,
}

// TOML `pin = 4711` (unquoted) must work like `pin = "4711"`.
fn string_or_int<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        I(u64),
    }
    Ok(match V::deserialize(d)? {
        V::S(s) => s,
        V::I(i) => i.to_string(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_missing_defaults_empty() {
        let c: ApiConfig =
            toml::from_str("enabled = true\nbind_address = \"0.0.0.0\"\nport = 8080").unwrap();
        assert_eq!(c.pin, "");
    }

    #[test]
    fn pin_unquoted_integer_parses() {
        let c: ApiConfig =
            toml::from_str("enabled = true\nbind_address = \"0.0.0.0\"\nport = 8080\npin = 4711")
                .unwrap();
        assert_eq!(c.pin, "4711");
    }
}
