use crate::workload::ClientRequest;
use crate::workload::ClientWorkItemKind;
use crate::workload::Generator;
use crate::*;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use std::time::Instant;
use tokio::runtime::Runtime;
use tonic::transport::Channel;

use pingpong::ping_client::PingClient;
use pingpong::PingRequest;

pub mod pingpong {
    tonic::include_proto!("pingpong");
}

// launch a pool manager and worker tasks since HTTP/2.0 is mux'ed we prepare
// senders in the pool manager and pass them over a queue to our worker tasks
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    generator: Generator,
    rng: &mut Xoshiro512PlusPlus,
) {
    debug!("launching ping grpc protocol tasks");

    for endpoint in config.target().endpoints() {
        for _ in 0..config.client().unwrap().poolsize() {
            while RUNNING.load(Ordering::Relaxed) {
                let endpoint = endpoint.clone();

                CONNECT.increment();

                let _ = runtime.enter();

                if let Ok(client) = runtime.block_on(async { PingClient::connect(endpoint).await })
                {
                    CONNECT_OK.increment();
                    CONNECT_CURR.increment();

                    // create one task per channel
                    for _ in 0..config.client().unwrap().concurrency() {
                        // Generate unique seed for this task
                        let mut seed = [0u8; 64];
                        rng.fill_bytes(&mut seed);

                        runtime.spawn(task(
                            config.clone(),
                            client.clone(),
                            generator.clone(),
                            Seed512(seed),
                        ));
                    }
                } else {
                    CONNECT_EX.increment();
                }
            }
        }
    }
}

async fn task(
    _config: Config,
    mut client: PingClient<Channel>,
    generator: Generator,
    seed: Seed512,
) -> Result<(), std::io::Error> {
    let mut rng = Xoshiro512PlusPlus::from_seed(seed);

    while RUNNING.load(Ordering::Relaxed) {
        // Wait for ratelimiter and generate request locally
        generator.wait();

        let work_item = match generator.generate_client_request(&mut rng) {
            Some(item) => item,
            None => continue,
        };

        REQUEST.increment();
        let start = Instant::now();
        #[allow(clippy::single_match)]
        let result = match work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                ClientRequest::Ping(_) => client
                    .ping(tonic::Request::new(PingRequest {}))
                    .await
                    .map(|_| ()),
                _ => {
                    REQUEST_UNSUPPORTED.increment();
                    continue;
                }
            },
            _ => {
                continue;
            }
        };

        REQUEST_OK.increment();

        let stop = Instant::now();

        match result {
            Ok(_) => {
                RESPONSE_OK.increment();

                let latency = stop.duration_since(start).as_nanos() as u64;

                let _ = RESPONSE_LATENCY.increment(latency);
            }
            Err(_) => {
                RESPONSE_EX.increment();
            }
        }
    }

    Ok(())
}
