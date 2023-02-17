// Copyright 2023 IOP Systems, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
// use crate::workload::*;
// use crate::workload::Keyspace;

use async_channel::{bounded, Sender};
// use rand::distributions::{Alphanumeric, Uniform};
use ::tokio::io::*;
use ::tokio::runtime::Builder;
use ::tokio::time::{sleep, Duration};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256Plus;
use ringlog::Drain;

use core::num::NonZeroU64;

mod drivers;
mod generators;

use self::generators::*;

// const CONNECTIONS: usize = 16;

// const PROTOCOL: Protocol = Protocol::Memcache;

// this should take some sort of configuration
pub fn run(config: Config, log: Box<dyn Drain>) -> Result<()> {
    let mut log = log;

    // Create the runtime
    let mut rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(config.general().threads())
        .build()?;

    // TODO: figure out what a reasonable size is here
    let (work_sender, work_receiver) = bounded(1_000_000);

    info!("Protocol: {}", config.general().protocol());

    match config.general().protocol() {
        Protocol::Memcache => {
            drivers::memcache::launch_tasks(&mut rt, config.clone(), work_receiver)
        }
        Protocol::Momento => drivers::momento::launch_tasks(&mut rt, config.clone(), work_receiver),
        Protocol::Ping => drivers::ping::launch_tasks(&mut rt, config.clone(), work_receiver),
        Protocol::Resp => drivers::resp::launch_tasks(&mut rt, config.clone(), work_receiver),
    }

    rt.spawn(requests(work_sender.clone(), config.clone()));

    rt.spawn(reconnect(work_sender, config.clone()));

    rt.spawn(async move {
        while RUNNING.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(50)).await;
            let _ = log.flush();
        }
        let _ = log.flush();
    });

    let window = config.general().interval() as u64;
    let mut interval = config.general().interval();
    let mut duration = config.general().duration();

    let mut window_id = 0;

    while duration > 0 {
        rt.block_on(async {
            sleep(Duration::from_secs(1)).await;
        });

        interval = interval.saturating_sub(1);
        duration = duration.saturating_sub(1);

        if interval == 0 {
            info!("-----");
            info!("Window: {}", window_id);
            let connect_ok = CONNECT_OK.reset();
            let connect_ex = CONNECT_EX.reset();
            let connect_timeout = CONNECT_TIMEOUT.reset();
            let connect_total = CONNECT.reset();

            let connect_sr = connect_ok as f64 / connect_total as f64;

            info!(
                "Connection: Open: {} Success Rate: {:.2} %",
                CONNECT_CURR.value(),
                connect_sr
            );
            info!(
                "Connection Rates (/s): Attempt: {:.2} Opened: {:.2} Errors: {:.2} Timeout: {:.2}",
                connect_total as f64 / window as f64,
                connect_ok as f64 / window as f64,
                connect_ex as f64 / window as f64,
                connect_timeout as f64 / window as f64,
            );

            let response_ok = RESPONSE_OK.reset();
            let response_ex = RESPONSE_EX.reset();
            let response_timeout = RESPONSE_TIMEOUT.reset();

            let response_total = response_ok + response_ex + response_timeout;

            let response_sr = 100.0 * response_ok as f64 / response_total as f64;
            let response_to = 100.0 * response_timeout as f64 / response_total as f64;

            info!(
                "Response: Success: {:.2} % Timeout: {:.2} %",
                response_sr, response_to
            );
            info!(
                "Response Rate (/s): Ok: {:.2} Error: {:.2} Timeout: {:.2}",
                response_ok as f64 / window as f64,
                response_ex as f64 / window as f64,
                response_timeout as f64 / window as f64,
            );

            let get_total = GET.reset() as f64;
            let get_ex = GET_EX.reset() as f64;
            let get_hit = GET_KEY_HIT.reset() as f64;
            let get_miss = GET_KEY_MISS.reset() as f64;
            let get_hr = 100.0 * get_hit / (get_hit + get_miss);
            let get_sr = 100.0 - (100.0 * get_ex / get_total);
            info!(
                "\tGet: rate (/s): {:.2} hit rate(%): {:.2} success rate(%): {:.2}",
                get_total / window as f64,
                get_hr,
                get_sr,
            );
            let set_total = SET.reset() as f64;
            let set_stored = SET_STORED.reset() as f64;
            let set_sr = (set_stored / set_total) * 100.0;
            info!(
                "\tSet: rate (/s): {:.2} success rate(%): {:.2}",
                set_total / window as f64,
                set_sr,
            );
            info!(
                "response rate (/s): ok: {:.2} error: {:.2} timeout: {:.2}",
                RESPONSE_OK.reset() / window,
                RESPONSE_EX.reset() / window,
                RESPONSE_TIMEOUT.reset() / window,
            );
            info!(
                "response latency (us): p25: {} p50: {} p75: {} p90: {} p99: {} p999: {} p9999: {}",
                RESPONSE_LATENCY
                    .percentile(25.0)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(50.0)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(75.0)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(90.0)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(99.0)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(99.9)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
                RESPONSE_LATENCY
                    .percentile(99.99)
                    .map(|b| format!("{}", b.high() / 1000))
                    .unwrap_or_else(|_| "ERR".to_string()),
            );

            interval = config.general().interval();
            window_id += 1;
        }
    }

    RUNNING.store(false, Ordering::Relaxed);

    Ok(())
}
