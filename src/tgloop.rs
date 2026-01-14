use caramel::ns::api::Client;
use log::{info, warn};
use std::{collections::VecDeque, sync::Arc, time::{Duration, Instant}};
use tokio::sync::{Mutex, mpsc};

use crate::api::send_telegram;

#[derive(Debug)]
pub struct Telegram {
    pub nation: String,
    pub tgid: String,
    pub tg_key: String,
    pub client_key: String,
}

impl Telegram {
    pub fn new(nation: String, tgid: String, tg_key: String, client_key: String) -> Self {
        Self { nation, tgid, tg_key, client_key }
    }
}

pub struct TelegramQueue {
    queue: VecDeque<Telegram>,
    identifier: String,
    ephemeral: bool,
    recruitment: bool,
}

impl TelegramQueue {
    pub fn new(identifier: String, ephemeral: bool, recruitment: bool) -> Self {
        Self { queue: VecDeque::new(), identifier, ephemeral, recruitment }
    }

    pub fn is_recruitment(&self) -> bool {
        self.recruitment
    }

    pub fn enqueue_tg(&mut self, telegram: Telegram) {
        if self.ephemeral {
            self.queue.clear();
        }

        self.queue.push_back(telegram);
    }

    pub fn dequeue_tg(&mut self) -> Option<Telegram> {
        self.queue.pop_back()
    }
}

pub struct TelegramState {
    queues: Vec<TelegramQueue>,
    signal: Option<mpsc::Sender<()>>,
}

impl TelegramState {
    pub fn new() -> Self {
        let mut state = Self { queues: Vec::new(), signal: None };
        state.queues.push(TelegramQueue::new("recruit-permanent".into(), false, true));
        state.queues.push(TelegramQueue::new("recruit-ephemeral".into(), true, true));
        state.queues.push(TelegramQueue::new("regional".into(), false, false));
        state
    }

    pub async fn add_to_queue(&mut self, queue_name: &str, telegram: Telegram) -> bool {
        for queue in &mut self.queues {
            if queue.identifier == queue_name {
                queue.enqueue_tg(telegram);

                if let Some(signal) = &mut self.signal {
                    signal.send(()).await.unwrap_or_else(|err| {
                        warn!("Error notifying telegram loop of new nation: {err:?}");
                    });
                }

                return true;
            }
        }

        return false;
    }
}

const RECRUITMENT_TELEGRAM_INTERVAL: u64 = 181;
const NORMAL_TELEGRAM_INTERVAL: u64 = 61;

fn can_recruit(last_recruitment_time: &Instant) -> bool {
    calculate_recruit_delay(last_recruitment_time).is_none()
}

fn calculate_recruit_delay(last_recruitment_time: &Instant) -> Option<Duration> {
    let time_since_last_recruit = Instant::now().duration_since(*last_recruitment_time).as_secs();

    if time_since_last_recruit >= RECRUITMENT_TELEGRAM_INTERVAL {
        return None;
    }

    Some(Duration::from_secs(RECRUITMENT_TELEGRAM_INTERVAL - time_since_last_recruit))
}

async fn telegram_loop(client: Arc<Client>, state: Arc<Mutex<TelegramState>>) {
    let mut last_recruitment_time = Instant::now();

    let (tx, mut rx) = mpsc::channel(100);

    {
        let mut state = state.lock().await;
        state.signal = Some(tx);
    }

    loop {
        let mut state = state.lock().await;

        // Clear the signal queue
        while let Ok(_) = rx.try_recv() {}

        let mut sent = false;
        let mut recruit_delay: Option<Duration> = None;

        for queue in &mut state.queues {
            if queue.is_recruitment() && !can_recruit(&last_recruitment_time) { 
                recruit_delay = calculate_recruit_delay(&last_recruitment_time);
                continue;
            }

            if let Some(telegram) = queue.dequeue_tg() {
                info!("Sending telegram {} to nation {} ({})", telegram.tgid, telegram.nation, &queue.identifier);
                last_recruitment_time = Instant::now();

                send_telegram(&client, telegram).await.unwrap_or_else(|err| {
                    warn!("Error sending telegram: {err:?}");
                });

                sent = true;
                break;
            }
        }

        drop(state); // Unlock mutex before blocking

        if !sent {
            // Wait for a nation to be added to queue, or for the recruitment timeout to expire, whichever happens first
            if let Some(delay) = recruit_delay {
                tokio::select! {
                    _ = rx.recv() => {},
                    _ = tokio::time::sleep(delay) => {},
                }
            } else {
                rx.recv().await;
            }
        } else {
            tokio::time::sleep(Duration::from_secs(NORMAL_TELEGRAM_INTERVAL)).await;
        }
    }
}

pub fn start_telegram_loop(client: Arc<Client>, state: Arc<Mutex<TelegramState>>) {
    tokio::spawn(async { telegram_loop(client, state).await; });
}