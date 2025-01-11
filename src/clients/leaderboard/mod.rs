//! This module is for services which provide an API for Leaderboard services.
//! One example is Momento's serverless Leaderboard API (http://gomomento.com)

mod momento;

use crate::workload::LeaderboardRequest;
use crate::*;

use async_channel::Receiver;
use tokio::runtime::Runtime;
use workload::ClientWorkItemKind;

use metriken::metric;
use metriken::AtomicHistogram;
use metriken::Counter;
use metriken::LazyCounter;

pub fn launch(
    config: &Config,
    work_receiver: Receiver<ClientWorkItemKind<LeaderboardRequest>>,
) -> Option<Runtime> {
    let client_config = match config.leaderboard_client() {
        Some(c) => c,
        None => {
            debug!("No client configuration specified");
            return None;
        }
    };

    debug!("Launching leaderboard clients...");

    // spawn the request drivers on their own runtime
    let mut client_rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(client_config.threads())
        .build()
        .expect("failed to initialize tokio runtime");

    match config.general().protocol() {
        Protocol::Momento => momento::launch_tasks(&mut client_rt, config.clone(), work_receiver),
        protocol => {
            eprintln!(
                "leaderboards are not supported for the {:?} protocol",
                protocol
            );
            std::process::exit(1);
        }
    }

    Some(client_rt)
}

/*
 * Metrics definitions for all leaderboard clients
 */
#[metric(
    name = "leaderboard/response/latency",
    metadata = { unit = "nanoseconds" }
)]
pub static LEADERBOARD_RESPONSE_LATENCY: AtomicHistogram = AtomicHistogram::new(7, 64);

#[metric(
    name = "leaderboard/response/ttfb",
    metadata = { unit = "nanoseconds" }
)]
pub static LEADERBOARD_RESPONSE_TTFB: AtomicHistogram = AtomicHistogram::new(7, 64);

#[metric(name = "leaderboard/connect/total")]
pub static LEADERBOARD_CONNECT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/connect/ok")]
pub static LEADERBOARD_CONNECT_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/total")]
pub static LEADERBOARD_REQUEST_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/ok")]
pub static LEADERBOARD_REQUEST_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/unsupported")]
pub static LEADERBOARD_REQUEST_UNSUPPORTED: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/reconnect")]
pub static LEADERBOARD_REQUEST_RECONNECT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/response/ok")]
pub static LEADERBOARD_RESPONSE_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/response/exception")]
pub static LEADERBOARD_RESPONSE_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/response/timeout")]
pub static LEADERBOARD_RESPONSE_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/response/backend_timeout")]
pub static LEADERBOARD_RESPONSE_BACKEND_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/response/ratelimited")]
pub static LEADERBOARD_RESPONSE_RATELIMITED: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/connections/current")]
pub static LEADERBOARD_CONNECTIONS_CURR: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/total")]
pub static LEADERBOARD_GET_RANK_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/ok")]
pub static LEADERBOARD_GET_RANK_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/exception")]
pub static LEADERBOARD_GET_RANK_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/timeout")]
pub static LEADERBOARD_GET_RANK_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/total")]
pub static LEADERBOARD_UPSERT_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/ok")]
pub static LEADERBOARD_UPSERT_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/exception")]
pub static LEADERBOARD_UPSERT_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/timeout")]
pub static LEADERBOARD_UPSERT_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);
