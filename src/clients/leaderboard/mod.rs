/// # Leaderboard Clients
///
/// Leaderboards provide at-scale competitor ranking and lookup.
///
/// RPC-Perf store clients are used to evaluate the performance of object
/// leaderboard services in terms of throughput and latency.
use crate::workload::Generator;
use crate::*;

use rand::SeedableRng;
use rand_xoshiro::Xoshiro512PlusPlus;
use tokio::runtime::Runtime;

mod momento;

pub fn launch(config: &Config, generator: Generator) -> Option<Runtime> {
    if config.leaderboard().is_none() {
        debug!("No leaderboard configuration specified");
        return None;
    }
    debug!("Launching clients...");

    config.leaderboard()?;

    // spawn the request drivers on their own runtime
    let mut client_rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(config.leaderboard().unwrap().threads())
        .build()
        .expect("failed to initialize tokio runtime");

    // Initialize a master RNG to generate unique seeds for each task
    let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

    match config.general().protocol() {
        Protocol::Momento => {
            momento::launch_tasks(&mut client_rt, config.clone(), generator, &mut rng)
        }
        protocol => {
            eprintln!(
                "leaderboard commands are not supported for the {:?} protocol",
                protocol
            );
        }
    }

    Some(client_rt)
}
