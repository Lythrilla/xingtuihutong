use anyhow::{Context, Result};
use std::{env, net::SocketAddr, str::FromStr};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_address: SocketAddr,
    pub admin_username: String,
    pub admin_password: String,
    pub allowed_origin: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/xingtuihutong.db".into());
        let bind_address = SocketAddr::from_str(
            &env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".into()),
        )
        .context("invalid BIND_ADDRESS")?;
        let admin_username = env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".into());
        let admin_password =
            env::var("ADMIN_PASSWORD").context("ADMIN_PASSWORD must be configured")?;
        let allowed_origin = env::var("ALLOWED_ORIGIN").unwrap_or_else(|_| "*".into());

        Ok(Self {
            database_url,
            bind_address,
            admin_username,
            admin_password,
            allowed_origin,
        })
    }
}
