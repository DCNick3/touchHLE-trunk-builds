use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub github: Github,
    pub builds: Builds,
    pub http: Http,
}

impl Config {
    pub fn load(environment: &str) -> Result<Config> {
        let config = config::Config::builder()
            .add_source(config::File::new("config.yaml", config::FileFormat::Yaml).required(false))
            .add_source(
                config::File::new("config.local.yaml", config::FileFormat::Yaml).required(false),
            )
            .add_source(
                config::File::new(
                    &format!("config.{}.yaml", environment),
                    config::FileFormat::Yaml,
                )
                .required(false),
            )
            .add_source(
                config::File::new(
                    &format!("config.{}.local.yaml", environment),
                    config::FileFormat::Yaml,
                )
                .required(false),
            )
            .add_source(
                config::Environment::with_prefix("config")
                    .prefix_separator("_")
                    .separator("__")
                    .list_separator(","),
            )
            .build()
            .context("Building the config file")?;

        config
            .try_deserialize()
            .context("Deserializing config structure failed")
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Github {
    pub token: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Builds {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub artifact_prefix: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Http {
    pub port: u16,
}
