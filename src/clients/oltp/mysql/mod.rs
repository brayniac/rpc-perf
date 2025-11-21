use crate::workload::*;
use crate::*;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use sqlx::Connection;
use sqlx::Error;
use sqlx::MySql;
use sqlx::MySqlConnection;
use sqlx::QueryBuilder;
use std::io::Result;
use std::time::Instant;
use tokio::runtime::Runtime;

/// Launch tasks with one conncetion per task as ping protocol is not mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    generator: Generator,
    rng: &mut Xoshiro512PlusPlus,
) {
    info!("launching mysql protocol tasks");

    for _ in 0..config.oltp().unwrap().poolsize() {
        for endpoint in config.target().endpoints() {
            // Generate unique seed for this task
            let mut seed = [0u8; 64];
            rng.fill_bytes(&mut seed);

            runtime.spawn(task(
                endpoint.clone(),
                config.clone(),
                generator.clone(),
                Seed512(seed),
            ));
        }
    }
}

// a task for MySQL OLTP servers
#[allow(clippy::slow_vector_initialization)]
async fn task(
    endpoint: String,
    _config: Config,
    generator: Generator,
    seed: Seed512,
) -> Result<()> {
    let mut rng = Xoshiro512PlusPlus::from_seed(seed);
    let mut connection = None;

    while RUNNING.load(Ordering::Relaxed) {
        if connection.is_none() {
            OLTP_CONNECT.increment();
            connection = match MySqlConnection::connect(&endpoint).await {
                Ok(c) => {
                    OLTP_CONNECT_OK.increment();
                    OLTP_CONNECT_CURR.increment();
                    Some(c)
                }
                Err(e) => {
                    error!("error acquiring connection from pool: {e}");
                    None
                }
            };

            continue;
        };

        let mut c = connection.take().unwrap();

        // Wait for ratelimiter and generate request locally
        generator.wait();

        let work_item = match generator.generate_oltp_client_request(&mut rng) {
            Some(item) => item,
            None => {
                connection = Some(c);
                continue;
            }
        };

        match work_item {
            workload::ClientWorkItemKind::Request { request, .. } => match request {
                workload::OltpRequest::PointSelect(request) => {
                    OLTP_REQUEST_OK.increment();

                    let start = Instant::now();

                    let result = QueryBuilder::<MySql>::new(&format!(
                        "SELECT c FROM {} WHERE id = {}",
                        request.table, request.id
                    ))
                    .build()
                    .fetch_one(&mut c)
                    .await;

                    let latency = start.elapsed();

                    match result {
                        Ok(_) => {
                            OLTP_RESPONSE_OK.increment();

                            let _ = OLTP_RESPONSE_LATENCY.increment(latency.as_nanos() as _);
                        }
                        Err(Error::RowNotFound) => {
                            OLTP_RESPONSE_OK.increment();

                            let _ = OLTP_RESPONSE_LATENCY.increment(latency.as_nanos() as _);
                        }
                        Err(e) => {
                            debug!("request error: {e}");

                            OLTP_RESPONSE_EX.increment();
                            OLTP_CONNECT_CURR.decrement();
                            continue;
                        }
                    }
                }
            },
            workload::ClientWorkItemKind::Reconnect => {
                OLTP_CONNECT_CURR.decrement();
                continue;
            }
        }

        // Check if we should reconnect
        if generator.should_reconnect() {
            OLTP_CONNECT_CURR.decrement();
        } else {
            connection = Some(c);
        }
    }

    Ok(())
}
