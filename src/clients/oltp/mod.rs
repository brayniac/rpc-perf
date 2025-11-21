use crate::workload::Generator;
use crate::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro512PlusPlus;
use tokio::runtime::Runtime;

mod mysql;

pub fn launch(config: &Config, generator: Generator) -> Option<Runtime> {
    if config.oltp().is_none() {
        debug!("No oltp configuration specified");
        return None;
    }

    debug!("Launching oltp clients...");

    // spawn the request drivers on their own runtime
    let mut client_rt = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(config.oltp().unwrap().threads())
        .build()
        .expect("failed to initialize tokio runtime");

    // Initialize a master RNG to generate unique seeds for each task
    let mut rng = Xoshiro512PlusPlus::from_seed(config.general().initial_seed());

    match config.general().protocol() {
        Protocol::Mysql => mysql::launch_tasks(&mut client_rt, config.clone(), generator, &mut rng),
        protocol => {
            eprintln!("oltp is not supported for the {:?} protocol", protocol);
            std::process::exit(1);
        }
    }

    Some(client_rt)
}
