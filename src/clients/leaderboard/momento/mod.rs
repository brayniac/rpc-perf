//! This module provides support for Momento's Leaderboard API
//!
//! Docs are available here: https://docs.momentohq.com/leaderboards

use crate::clients::leaderboard::*;
use crate::clients::ResponseError;
use crate::config::workload::LeaderboardOrder;
use crate::workload::*;
use crate::*;

use ::momento::leaderboard::configurations::LowLatency;
use ::momento::*;
use async_channel::Receiver;
use tokio::runtime::Runtime;

use std::io::{Error, Result};
use std::time::Instant;

/// Launch tasks with one channel per task as Momento Client is mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    work_receiver: Receiver<ClientWorkItemKind<LeaderboardRequest>>,
) {
    debug!("launching momento protocol leaderboard tasks");

    let client_config = config.leaderboard_client().unwrap();

    for _ in 0..client_config.poolsize() {
        let client = {
            let _guard = runtime.enter();

            // initialize the Momento cache client
            if std::env::var("MOMENTO_API_KEY").is_err() {
                eprintln!("environment variable `MOMENTO_API_KEY` is not set");
                std::process::exit(1);
            }

            let credential_provider =
                match CredentialProvider::from_env_var("MOMENTO_API_KEY".to_string()) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("MOMENTO_API_KEY key should be valid: {e}");
                        std::process::exit(1);
                    }
                };

            match LeaderboardClient::builder()
                .configuration(LowLatency::latest())
                .credential_provider(credential_provider)
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("could not create client: {}", e);
                    std::process::exit(1);
                }
            }
        };

        LEADERBOARD_CONNECT.increment();
        LEADERBOARD_CONNECTIONS_CURR.increment();

        // create one task per channel
        for _ in 0..client_config.concurrency() {
            runtime.spawn(task(config.clone(), client.clone(), work_receiver.clone()));
        }
    }
}

async fn task(
    config: Config,
    mut client: LeaderboardClient,
    work_receiver: Receiver<ClientWorkItemKind<LeaderboardRequest>>,
) -> Result<()> {
    let client_config = config.leaderboard_client.clone().unwrap_or_else(|| {
        eprintln!("leaderboard configuration was not specified");
        std::process::exit(1);
    });
    let cache_name = config.target.cache_name.clone().unwrap_or_else(|| {
        eprintln!("store name is not specified in the `store` section");
        std::process::exit(1);
    });

    let request_timeout = client_config.request_timeout();

    while RUNNING.load(Ordering::Relaxed) {
        let work_item = work_receiver
            .recv()
            .await
            .map_err(|_| Error::other("channel closed"))?;

        LEADERBOARD_REQUEST_TOTAL.increment();

        let start = Instant::now();

        let result: std::result::Result<(), ResponseError> = match work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                LeaderboardRequest::GetRank {
                    leaderboard,
                    ids,
                    order,
                } => {
                    get_rank(
                        &mut client,
                        request_timeout,
                        cache_name.clone(),
                        leaderboard,
                        ids,
                        order,
                    )
                    .await
                }
                LeaderboardRequest::Upsert {
                    leaderboard,
                    elements,
                } => {
                    upsert(
                        &mut client,
                        request_timeout,
                        cache_name.clone(),
                        leaderboard,
                        elements,
                    )
                    .await
                }
            },
            ClientWorkItemKind::Reconnect => {
                continue;
            }
        };

        let stop = Instant::now();

        LEADERBOARD_REQUEST_OK.increment();

        match result {
            Ok(_) => {
                LEADERBOARD_RESPONSE_OK.increment();

                let latency = stop.duration_since(start).as_nanos() as u64;

                let _ = LEADERBOARD_RESPONSE_LATENCY.increment(latency);
            }
            Err(ResponseError::Exception) => {
                LEADERBOARD_RESPONSE_EX.increment();
            }
            Err(ResponseError::Timeout) => {
                LEADERBOARD_RESPONSE_TIMEOUT.increment();
            }
            Err(ResponseError::Ratelimited) => {
                LEADERBOARD_RESPONSE_RATELIMITED.increment();
            }
            Err(ResponseError::BackendTimeout) => {
                LEADERBOARD_RESPONSE_BACKEND_TIMEOUT.increment();
            }
        }
    }

    Ok(())
}

/*
 * Command Implementations
 */

/// Get the `RankedElement`s for a set of IDs
pub async fn get_rank(
    client: &mut LeaderboardClient,
    timeout: Duration,
    cache_name: String,
    leaderboard: String,
    ids: Vec<u32>,
    order: LeaderboardOrder,
) -> std::result::Result<(), ResponseError> {
    LEADERBOARD_GET_RANK_TOTAL.increment();

    let result = tokio::time::timeout(
        timeout,
        client.get_rank::<Vec<u32>>(cache_name, leaderboard, ids, order.into()),
    )
    .await;

    match result {
        Ok(Ok(_)) => {
            LEADERBOARD_GET_RANK_OK.increment();
        }
        Ok(Err(_)) => {
            LEADERBOARD_GET_RANK_EX.increment();
        }
        Err(_) => {
            LEADERBOARD_GET_RANK_TIMEOUT.increment();
        }
    }

    Ok(())
}

pub async fn upsert(
    client: &mut LeaderboardClient,
    timeout: Duration,
    cache_name: String,
    leaderboard: String,
    elements: Vec<(u32, f64)>,
) -> std::result::Result<(), ResponseError> {
    LEADERBOARD_UPSERT_TOTAL.increment();

    let result = tokio::time::timeout(
        timeout,
        client.upsert_elements::<Vec<(u32, f64)>>(cache_name, leaderboard, elements),
    )
    .await;

    match result {
        Ok(Ok(_)) => {
            LEADERBOARD_UPSERT_OK.increment();
        }
        Ok(Err(_)) => {
            LEADERBOARD_UPSERT_EX.increment();
        }
        Err(_) => {
            LEADERBOARD_UPSERT_TIMEOUT.increment();
        }
    }

    Ok(())
}
