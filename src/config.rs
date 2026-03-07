use indexmap::IndexMap;
use std::collections::HashMap;

#[derive(serde::Deserialize)]
pub struct Config {
    pub user: UserData,
    pub paperwork: HashMap<String, PaperMapping>,
}

#[derive(serde::Deserialize)]
pub struct UserData {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub organization: String,
}

#[derive(serde::Deserialize)]
pub struct PaperMapping {
    pub template: String,
    pub output_name: String,
    pub fields: IndexMap<String, Vec<String>>,
}

pub fn load(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let config = toml::from_str::<Config>(&contents)?;
    Ok(config)
}
