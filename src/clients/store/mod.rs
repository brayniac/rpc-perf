/// # Store Clients
///
/// Store clients are focused on object storage. This is distinct from caching
/// in that data is not evicted from an object store. Expiration may still be
/// possible with some implementations.
///
/// RPC-Perf store clients are used to evaluate the performance of object
/// storage services in terms of throughput and latency.
use crate::workload::Generator;
use crate::*;

use rand::SeedableRng;
use rand_xoshiro::Xoshiro512PlusPlus;
use tokio::runtime::Runtime;

mod s3;

pub fn launch(config: &Config, generator: Generator) -> Option<Runtime> {
    if config.storage().is_none() {
        debug!("No store configuration specified");
        return None;
    }
    debug!("Launching clients...");

    config.storage()?;

    // spawn the request drivers on their own runtime
    let mut client_rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(config.storage().unwrap().threads())
        .build()
        .expect("failed to initialize tokio runtime");

    // Initialize a master RNG to generate unique seeds for each task
    let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

    match config.general().protocol() {
        Protocol::S3 => s3::launch_tasks(&mut client_rt, config.clone(), generator, &mut rng),
        protocol => {
            eprintln!(
                "store commands are not supported for the {:?} protocol",
                protocol
            );
        }
    }

    Some(client_rt)
}
