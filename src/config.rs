use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize, Default)]
pub struct Config {
    pub default: Option<DefaultSection>,
    pub profile: Option<HashMap<String, Profile>>,
}

#[derive(Deserialize)]
pub struct DefaultSection {
    pub profile: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
pub struct Profile {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub model: Option<String>,
    pub path_prefix: Option<String>,
    pub api_key_env: Option<String>,
}

pub fn load() -> Config {
    let path = config_path();

    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn config_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(format!("{}/.void/config.toml", h)))
}

pub fn get_profile(config: &Config, name: &str) -> Option<Profile> {
    config
        .profile
        .as_ref()
        .and_then(|profiles| profiles.get(name).cloned())
}

pub fn get_default_profile_name(config: &Config) -> Option<String> {
    config
        .default
        .as_ref()
        .and_then(|default| default.profile.clone())
}
