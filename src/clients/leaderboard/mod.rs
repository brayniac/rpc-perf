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

/*
 * Per-Request type metrics
 */

// get rank

#[metric(name = "leaderboard/request/delete/total")]
pub static LEADERBOARD_DELETE_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/delete/ok")]
pub static LEADERBOARD_DELETE_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/delete/exception")]
pub static LEADERBOARD_DELETE_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/delete/timeout")]
pub static LEADERBOARD_DELETE_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);


// get rank

#[metric(name = "leaderboard/request/get_rank/total")]
pub static LEADERBOARD_GET_RANK_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/ok")]
pub static LEADERBOARD_GET_RANK_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/exception")]
pub static LEADERBOARD_GET_RANK_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_rank/timeout")]
pub static LEADERBOARD_GET_RANK_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

// get by rank

#[metric(name = "leaderboard/request/get_by_rank/total")]
pub static LEADERBOARD_GET_BY_RANK_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_rank/ok")]
pub static LEADERBOARD_GET_BY_RANK_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_rank/exception")]
pub static LEADERBOARD_GET_BY_RANK_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_rank/timeout")]
pub static LEADERBOARD_GET_BY_RANK_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

// get by score

#[metric(name = "leaderboard/request/get_by_score/total")]
pub static LEADERBOARD_GET_BY_SCORE_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_score/ok")]
pub static LEADERBOARD_GET_BY_SCORE_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_score/exception")]
pub static LEADERBOARD_GET_BY_SCORE_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/get_by_score/timeout")]
pub static LEADERBOARD_GET_BY_SCORE_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

// get rank

#[metric(name = "leaderboard/request/length/total")]
pub static LEADERBOARD_LENGTH_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/length/ok")]
pub static LEADERBOARD_LENGTH_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/length/exception")]
pub static LEADERBOARD_LENGTH_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/length/timeout")]
pub static LEADERBOARD_LENGTH_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

// remove

#[metric(name = "leaderboard/request/remove/total")]
pub static LEADERBOARD_REMOVE_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/remove/ok")]
pub static LEADERBOARD_REMOVE_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/remove/exception")]
pub static LEADERBOARD_REMOVE_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/remove/timeout")]
pub static LEADERBOARD_REMOVE_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);

// upsert

#[metric(name = "leaderboard/request/upsert/total")]
pub static LEADERBOARD_UPSERT_TOTAL: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/ok")]
pub static LEADERBOARD_UPSERT_OK: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/exception")]
pub static LEADERBOARD_UPSERT_EX: LazyCounter = LazyCounter::new(Counter::default);

#[metric(name = "leaderboard/request/upsert/timeout")]
pub static LEADERBOARD_UPSERT_TIMEOUT: LazyCounter = LazyCounter::new(Counter::default);
