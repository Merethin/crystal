use std::{sync::Arc, error::Error};

use axum::{
    Json, Router, extract::State, http::{HeaderMap, StatusCode}, response::IntoResponse, routing::post
};

use log::warn;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::tgloop::{Telegram, TelegramState};

#[derive(Debug, Deserialize)]
pub struct RequestQueryModel {
    queue: String,
    tgid: String,
    tg_key: String,
    client_key: String,
    nations: Vec<String>,
}

#[derive(Clone)]
struct ServerState {
    tg_state: Arc<Mutex<TelegramState>>,
    auth_key: String,
}

async fn add_telegram(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(params): Json<RequestQueryModel>,
) -> impl IntoResponse {
    let auth_header = headers.get("x-crystal-key").and_then(|header| header.to_str().ok());

    if auth_header != Some(&state.auth_key) {
        return (StatusCode::FORBIDDEN, "Invalid or missing key").into_response();
    }

    let mut state = state.tg_state.lock().await;

    for nation in params.nations {
        state.add_to_queue(&params.queue, 
            Telegram::new(nation, params.tgid.clone(), params.tg_key.clone(), params.client_key.clone())
        ).await;
    }

    (StatusCode::OK, "Success").into_response()
}

pub async fn start_api_server(
    state: Arc<Mutex<TelegramState>>,
    key: String,
) -> Result<(), Box<dyn Error>> {
    let app = Router::new()
        .route("/queue", post(add_telegram))
        .with_state(ServerState { tg_state: state, auth_key: key });

    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:6496").await.unwrap();
        axum::serve(listener, app.into_make_service()).await.unwrap_or_else(|err| {
            warn!("Error in server: {}", err);
        });
    });

    Ok(())
}