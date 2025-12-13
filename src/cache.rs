use std::{collections::{HashMap, HashSet}, sync::Arc, time::Duration};

use caramel::ns::api::Client;
use regex::{Error, Regex};
use tokio::sync::{RwLock, mpsc};

pub struct RegexCache {
    map: HashMap<String, Regex>
}

impl RegexCache {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn get_regex(&mut self, pattern: &str) -> Result<Regex, Error> {
        Ok(self.map.entry(pattern.to_string()).or_insert(Regex::new(pattern)?).clone())
    }
}

pub struct Cache {
    pub regex: RwLock<RegexCache>,
    pub wa_nations: RwLock<HashSet<String>>,
    pub wa_signal: mpsc::Sender<()>,
    pub client: Arc<Client>
}

pub fn spawn_wa_worker(
    client: Arc<Client>,
) -> Arc<Cache> {
    let (send, mut recv) = mpsc::channel::<()>(100);

    let cache = Arc::new(Cache {
        regex: RwLock::new(RegexCache::new()),
        wa_nations: RwLock::new(HashSet::new()),
        wa_signal: send,
        client: client.clone()
    });

    let cache_clone = cache.clone();
    let _ = tokio::spawn(async move {
        while let Some(_) = recv.recv().await {
            loop {
                let mut wa_nations = cache_clone.wa_nations.write().await;

                if let Err(_) = crate::api::query_wa_nations(&client, &mut wa_nations).await {
                    drop(wa_nations);
                    tokio::time::sleep(Duration::from_secs(120)).await; // Try again after 2 minutes
                } else {
                    break;
                }
            }
        }
    });

    cache
}