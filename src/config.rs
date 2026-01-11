use log::{error, warn};
use serde::Deserialize;
use std::{collections::HashMap, fs, process::exit};
use toml::{Table, Value};

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub event: Vec<String>,
    pub regions: Vec<String>,
    pub nations: Vec<String>,
    pub queue: String,
    pub templates: Vec<String>
}

#[derive(Debug, Deserialize)]
pub struct TemplateConfig {
    pub tgid: String,
    pub tg_key: String,
    pub client_key: String,
}

#[derive(Debug)]
pub struct InputConfig {
    pub exchange_name: String,
}

#[derive(Debug)]
pub struct Config {
    pub input: InputConfig,
    pub templates: HashMap<String, TemplateConfig>,
    pub rules: Vec<(String, Rule)>,
}

fn parse_template(name: &str, table: &Table) -> Option<TemplateConfig> {
    let mut result = TemplateConfig { tgid: "".into(), tg_key: "".into(), client_key: "".into() };

    for (key, value) in table.iter() {
        if key == "tgid" && let toml::Value::String(v) = value {
            result.tgid = v.clone();
        } else if key == "tg_key" && let toml::Value::String(v) = value {
            result.tg_key = v.clone();
        } else if key == "client_key" && let toml::Value::String(v) = value {
            result.client_key = v.clone();
        } else {
            warn!("Unrecognized config key {} in template {}", key, name);
            return None;
        }
    }

    if result.tgid.is_empty() || result.tg_key.is_empty() || result.client_key.is_empty() {
        warn!("Template {} is missing fields", name);
        None
    } else {
        Some(result)
    }
}

fn parse_template_map(table: &Table) -> HashMap<String, TemplateConfig> {
    let mut result = HashMap::new();

    for (key, value) in table.iter() {
        if let toml::Value::Table(t) = value {
            if let Some(template) = parse_template(key, t) {
                result.insert(key.clone(), template);
            } else {
                warn!("Couldn't parse template '{}'", key);
            }
        }
    }

    result
}

fn convert_toml_array_to_string_vec(array: &Vec<Value>) -> Vec<String> {
    array.iter().flat_map(
        |v| v.as_str().and_then(
            |v| Some(v.to_string())
        )
    ).collect()
}

fn parse_rule(table: &Table) -> Rule {
    let mut result = Rule { 
        event: Vec::new(),
        regions: Vec::new(),
        nations: Vec::new(),
        queue: "".into(),
        templates: Vec::new(),
    };

    if let Some(toml::Value::Array(s)) = table.get("event") {
        result.event = convert_toml_array_to_string_vec(s);
    }

    if let Some(toml::Value::Array(s)) = table.get("regions") {
        result.regions = convert_toml_array_to_string_vec(s);
    }

    if let Some(toml::Value::Array(s)) = table.get("nations") {
        result.nations = convert_toml_array_to_string_vec(s);
    }
    
    if let Some(toml::Value::String(s)) = table.get("queue") {
        result.queue = s.clone();
    }

    if let Some(toml::Value::Array(s)) = table.get("templates") {
        result.templates = convert_toml_array_to_string_vec(s);
    }

    result
}

fn parse_rules(table: &Table) -> Vec<(String, Rule)> {
    let mut result = Vec::new();

    for (key, value) in table.iter() {
        if let toml::Value::Table(v) = value {
            result.push((key.clone(), parse_rule(v)));
        }
    }

    result
}

pub fn parse_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let table: toml::Table = toml::from_str(&contents.as_str())?;

    let input: InputConfig = match table.get("input") {
        Some(toml::Value::Table(t)) => {
            let exchange_name = match t.get("exchange_name") {
                Some(toml::Value::String(s)) => s.clone(),
                _ => {
                    error!("Config is missing required 'input.exchange_name' value!");
                    exit(1);
                }
            };

            InputConfig { exchange_name }
        },
        _ => {
            error!("Config is missing required 'input' section!");
            exit(1);
        }
    };

    let templates = match table.get("templates") {
        Some(toml::Value::Table(t)) => {
            parse_template_map(t)
        },
        _ => {
            warn!("No templates specified in config!");
            HashMap::new()
        }
    };

    let rules = match table.get("rules") {
        Some(toml::Value::Table(t)) => {
            parse_rules(t)
        },
        _ => {
            warn!("No rules specified in config!");
            Vec::new()
        }
    };

    Ok(Config { input, templates, rules })
}