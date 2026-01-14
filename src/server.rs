use std::{sync::Arc, error::Error};

use log::error;
use lapin::{Channel, Consumer, options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions}, types::FieldTable};
use serde::Deserialize;
use tokio::sync::Mutex;
use futures_util::StreamExt;

use crate::tgloop::{Telegram, TelegramState};

#[derive(Debug, Deserialize)]
pub struct RequestedTelegram {
    queue: String,
    nation: String,
    tgid: String,
    tg_key: String,
    client_key: String,
}

async fn server_loop(
    consumer: Consumer,
    state: Arc<Mutex<TelegramState>>
) {
    consumer.for_each_concurrent(None, move |delivery| {
        let state = state.clone();

        async move {
            let Ok(delivery) = delivery else { return; };

            if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
                error!("Error while acknowledging delivery: {}", err);
            }

            if let Some(tg) = serde_json::from_slice::<RequestedTelegram>(&delivery.data).ok() {
                state.lock().await.add_to_queue(&tg.queue, Telegram::new(
                    tg.nation, tg.tgid, tg.tg_key, tg.client_key
                )).await;
            }
        }
    }).await;
}

pub async fn start_server_loop(
    channel: &Channel,
    state: Arc<Mutex<TelegramState>>
) -> Result<(), Box<dyn Error>> {
    let queue = channel.queue_declare(
        "crystal_server", 
        QueueDeclareOptions::default(), 
        FieldTable::default()
    ).await?;

    let consumer = channel.basic_consume(
        queue.name().as_str(), 
        "consumer_server", 
        BasicConsumeOptions::default(),
        FieldTable::default()
    ).await?;

    tokio::spawn(async { server_loop(consumer, state).await; });

    Ok(())
}