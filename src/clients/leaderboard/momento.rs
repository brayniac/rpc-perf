use super::record_result;
use crate::clients::ResponseError;
use crate::config::Config;
use crate::metrics::*;
use crate::workload::{ClientWorkItemKind, Generator, LeaderboardClientRequest};
use crate::{workload, RUNNING};
use paste::paste;

use momento::leaderboard::{configurations, LeaderboardClient};
use momento::CredentialProvider;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use ringlog::debug;
use tokio::runtime::Runtime;
use tokio::time::timeout;

use std::io::Result;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Launch tasks with one channel per task as gRPC is mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    generator: Generator,
    rng: &mut Xoshiro512PlusPlus,
) {
    debug!("launching momento protocol tasks");

    let cache_name = config
        .target()
        .cache_name()
        .unwrap_or_else(|| {
            eprintln!("cache name is not specified in the `target` section");
            std::process::exit(1);
        })
        .to_string();

    for _ in 0..config.leaderboard().unwrap().poolsize() {
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
                .configuration(configurations::LowLatency::v1())
                .credential_provider(credential_provider)
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("could not create leaderboard client: {}", e);
                    std::process::exit(1);
                }
            }
        };

        LEADERBOARD_CONNECT.increment();
        LEADERBOARD_CONNECT_CURR.increment();

        // create one task per channel
        for _ in 0..config.leaderboard().unwrap().concurrency() {
            // Generate unique seed for this task
            let mut seed = [0u8; 64];
            rng.fill_bytes(&mut seed);

            runtime.spawn(task(
                config.clone(),
                client.clone(),
                cache_name.clone(),
                generator.clone(),
                Seed512(seed),
            ));
        }
    }
}

async fn task(
    config: Config,
    mut client: LeaderboardClient,
    cache_name: String,
    generator: Generator,
    seed: Seed512,
) -> Result<()> {
    let mut rng = Xoshiro512PlusPlus::from_seed(seed);

    while RUNNING.load(Ordering::Relaxed) {
        // Wait for ratelimiter and generate request locally
        generator.wait();

        let work_item = match generator.generate_leaderboard_client_request(&mut rng) {
            Some(item) => item,
            None => continue,
        };

        LEADERBOARD_REQUEST.increment();
        let start = Instant::now();
        let result = match work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                LeaderboardClientRequest::Upsert(r) => {
                    upsert(&mut client, &config, cache_name.clone(), r).await
                }
                LeaderboardClientRequest::GetCompetitionRank(r) => {
                    get_competition_rank(&mut client, &config, cache_name.clone(), r).await
                }
                _ => {
                    LEADERBOARD_REQUEST_UNSUPPORTED.increment();
                    continue;
                }
            },
            ClientWorkItemKind::Reconnect => {
                continue;
            }
        };

        LEADERBOARD_REQUEST_OK.increment();

        let stop = Instant::now();

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

/// Insert or update id-score pairs in the leaderboard.
pub async fn upsert(
    client: &mut LeaderboardClient,
    config: &Config,
    cache_name: String,
    request: workload::leaderboard::Upsert,
) -> std::result::Result<(), ResponseError> {
    LEADERBOARD_UPSERT.increment();

    let leaderboard = client.leaderboard(cache_name, request.leaderboard.as_ref().clone());
    let result = timeout(
        config.leaderboard().unwrap().request_timeout(),
        leaderboard.upsert(request.elements),
    )
    .await;

    record_result!(result, LEADERBOARD_UPSERT)
}

/// Get the competition rank of a list of ids in a leaderboard.
pub async fn get_competition_rank(
    client: &mut LeaderboardClient,
    config: &Config,
    cache_name: String,
    request: workload::leaderboard::GetCompetitionRank,
) -> std::result::Result<(), ResponseError> {
    LEADERBOARD_GET_COMPETITION_RANK.increment();

    let leaderboard = client.leaderboard(cache_name, request.leaderboard.as_ref().clone());
    let ids = request.ids.as_ref().to_vec();
    let result = timeout(
        config.leaderboard().unwrap().request_timeout(),
        leaderboard.get_competition_rank(ids),
    )
    .await;

    record_result!(result, LEADERBOARD_GET_COMPETITION_RANK)
}
