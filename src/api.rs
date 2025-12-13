use log::{info, warn};
use std::collections::HashSet;
use serde::Deserialize;

use caramel::ns::api::{Client, ApiError};
use caramel::ns::xml::parse_wa_members;

use crate::tgloop::Telegram;

pub async fn query_wa_nations(
    client: &Client, set: &mut HashSet<String>
) -> Result<(), ApiError> {
    let response = client.make_request_with_retry(vec![
            ("wa", "1"), ("q", "members")
        ]).await?;

    if let Ok(members) = parse_wa_members(&response) {
        set.clear();
        for nation in members {
            set.insert(nation);
        }

        info!("Queried {} WA nations", set.len());
    } else {
        warn!("Invalid XML from WA members API request");
    }

    return Ok(());
}

pub async fn send_telegram(
    client: &Client, telegram: Telegram
) -> Result<(), ApiError> {
    let response = client.make_request_with_retry(vec![
            ("a", "sendTG"), 
            ("client", &telegram.client_key),
            ("tgid", &telegram.tgid),
            ("key", &telegram.tg_key),
            ("to", &telegram.nation)
        ]).await?;

    if response.contains("Client Not Registered For API") {
        warn!("Error while sending telegram: Invalid client key!");
    }

    return Ok(());
}

#[derive(Deserialize)]
struct CanRecruitRoot {
    #[serde(rename = "TGCANRECRUIT")]
    pub can_recruit: String,
}

pub async fn can_telegram(
    client: &Client, nation: &str
) -> bool {
    if let Ok(response) = client.make_request(vec![
        ("nation", nation), ("q", "tgcanrecruit")
    ]).await {
        return quick_xml::de::from_str::<CanRecruitRoot>(&response).and_then(
            |v| Ok(&v.can_recruit == "1")
        ).unwrap_or(false);
    }

    return false;
}
