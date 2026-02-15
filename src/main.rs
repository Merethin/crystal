mod tgloop;
mod config;
mod rules;
mod server;
mod api;
mod cache;

use std::{error::Error, sync::Arc, process::exit};
use rand::{rngs::ThreadRng, seq::IndexedRandom};
use tokio::sync::Mutex;
use log::{error, info};

use caramel::{ns::{UserAgent, api::Client}, akari, log::setup_log, types::akari::Event};

use crate::{cache::{Cache, spawn_wa_worker}, server::start_api_server};
use crate::tgloop::{Telegram, TelegramState, start_telegram_loop};
use crate::config::{Config, parse_config};

const PROGRAM: &str = "crystal";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Merethin";
const CONFIG_PATH: &'static str = "config/crystal.toml";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_log(vec![]);

    dotenv::dotenv().ok();

    let user_agent = UserAgent::read_from_env(PROGRAM, VERSION, AUTHOR);

    let config = parse_config(CONFIG_PATH).unwrap_or_else(|err| {
        error!("Failed to read config file: {}", err);
        exit(1);
    });

    let url = std::env::var("RABBITMQ_URL").unwrap_or_else(|err| {
        error!("Missing RABBITMQ_URL environment variable: {err}");
        exit(1);
    });

    let auth_key = std::env::var("CRYSTAL_AUTH_KEY").unwrap_or_else(|err| {
        error!("Missing CRYSTAL_AUTH_KEY environment variable: {err}");
        exit(1);
    });

    let conn = lapin::Connection::connect(
        &url,
        lapin::ConnectionProperties::default(),
    ).await?;

    let channel = conn.create_channel().await?;
    let mut consumer = akari::create_consumer(&channel, &config.input.exchange_name, None).await?;

    let client = Arc::new(Client::new(user_agent).unwrap_or_else(|err| {
        error!("Failed to initialize API client: {}", err);
        exit(1);
    }));

    let state = Arc::new(Mutex::new(TelegramState::new()));
    let cache = spawn_wa_worker(client.clone());

    cache.wa_signal.send(()).await.unwrap_or_else(|err| {
        error!("Failed to trigger WA nation update: {err}");
    });

    start_telegram_loop(client.clone(), state.clone());
    start_api_server(state.clone(), auth_key).await?;

    let mut rng = rand::rng();
    while let Some(event) = akari::consume(&mut consumer).await {
        process_event(&config, state.clone(), event, cache.clone(), &mut rng).await;
    }

    Ok(())
}

pub async fn process_event(
    config: &Config,
    state: Arc<Mutex<TelegramState>>, 
    event: Event,
    cache: Arc<Cache>,
    rng: &mut ThreadRng,
) {
    if event.category == "connmiss" {
        cache.wa_signal.send(()).await.unwrap_or_else(|err| {
            error!("Failed to trigger WA nation update: {err}");
        });
    }

    update_wa(&event, cache.clone()).await;

    for (rule_name, rule) in &config.rules {
        if rules::match_rule(&event, rule, cache.clone()).await {
            if let Some(template) = rule.templates.choose(rng).and_then(
                |key| config.templates.get(key)
            ) && let Some(nation) = &event.actor {
                let mut state = state.lock().await;
                let success = state.add_to_queue(&rule.queue, Telegram::new(
                    nation.clone(), template.tgid.clone(), 
                    template.tg_key.clone(), template.client_key.clone()
                )).await;

                if success {
                    info!("Nation '{}' added to queue '{}', matching rule '{}'", nation, rule.queue, rule_name);
                }
            }

            break;
        }
    }
}

async fn update_wa(event: &Event, cache: Arc<Cache>) {
    match event.category.as_str() {
        "ncte" => {
            if let Some(nation) = &event.receptor {
                cache.wa_nations.write().await.remove(nation);
            }
        },
        "wadmit" => {
            if let Some(nation) = &event.actor {
                cache.wa_nations.write().await.insert(nation.clone());
            }
        },
        "wresign" => {
            if let Some(nation) = &event.actor {
                cache.wa_nations.write().await.remove(nation);
            }
        },
        "wkick" => {
            if let Some(nation) = &event.receptor {
                cache.wa_nations.write().await.remove(nation);
            }
        },
        _ => {},
    }
}