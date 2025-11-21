use crate::{
    replay::replay_engine::launch_replay_workload,
    workload::{Generator, Ratelimit},
};
use async_channel::bounded;
use backtrace::Backtrace;
use clap::{Arg, Command};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use once_cell::sync::Lazy;
use ringlog::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::sync::RwLock;
use tokio::time::sleep;

mod admin;
mod clients;
mod config;
mod metrics;
mod net;
mod output;
mod replay;
mod workload;

use config::{Protocol, *};
use metrics::*;

static RUNNING: AtomicBool = AtomicBool::new(true);
static WAIT: AtomicUsize = AtomicUsize::new(0);

static METRICS_SNAPSHOT: Lazy<Arc<RwLock<MetricsSnapshot>>> =
    Lazy::new(|| Arc::new(RwLock::new(Default::default())));

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        eprintln!("{s}");
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    #[allow(clippy::expect_used)]
    tokio_rustls::rustls::crypto::CryptoProvider::install_default(
        tokio_rustls::rustls::crypto::aws_lc_rs::default_provider(),
    )
    .expect("rustls _why_");

    // parse command line options
    let matches = Command::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .long_about(
            "A benchmarking, load generation, and traffic replay tool for RPC \
            services.",
        )
        .arg(
            Arg::new("CONFIG")
                .help("Server configuration file")
                .action(clap::ArgAction::Set)
                .index(1),
        )
        .subcommand(Command::new("replay").about("Replay a command log to generate load"))
        .get_matches();

    // load config from file
    let config = if let Some(file) = matches.get_one::<String>("CONFIG") {
        Config::new(file)
    } else {
        eprintln!("configuration file not provided");
        std::process::exit(1);
    };

    output!("Protocol: {:?}", config.general().protocol());
    output!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // configure debug log
    let debug_output: Box<dyn Output> = if let Some(file) = config.debug().log_file() {
        let backup = config
            .debug()
            .log_backup()
            .unwrap_or(format!("{}.old", file));
        Box::new(
            File::new(&file, &backup, config.debug().log_max_size())
                .expect("failed to open debug log file"),
        )
    } else {
        // by default, log to stderr
        Box::new(Stderr::new())
    };

    let level = config.debug().log_level();

    let debug_log = if level <= Level::Info {
        LogBuilder::new().format(ringlog::default_format)
    } else {
        LogBuilder::new()
    }
    .output(debug_output)
    .log_queue_depth(config.debug().log_queue_depth())
    .single_message_size(config.debug().log_single_message_size())
    .build()
    .expect("failed to initialize debug log");

    let mut log = MultiLogBuilder::new()
        .level_filter(config.debug().log_level().to_level_filter())
        .default(debug_log)
        .build()
        .start();

    // initialize async runtime for control plane
    let control_runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("failed to initialize tokio runtime");

    // spawn logging thread
    control_runtime.spawn(async move {
        while RUNNING.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(1)).await;
            let _ = log.flush();
        }
        let _ = log.flush();
    });

    // spawn thread to maintain histogram snapshots
    {
        let interval = config.general().interval();
        control_runtime.spawn(async move {
            while RUNNING.load(Ordering::Relaxed) {
                // acquire a lock and update the snapshots
                {
                    let mut snapshots = METRICS_SNAPSHOT.write().await;
                    snapshots.update();
                }

                // delay until next update
                sleep(interval).await;
            }
        });
    }

    // switch into replay mode if the replay subcommand is provided
    if matches.subcommand_matches("replay").is_some() {
        if config.replay().is_none() {
            eprintln!("replay configuration section is required for replay mode");
            std::process::exit(1);
        }

        info!("Starting replay mode");

        // Queue depth per client thread for replay mode
        const REPLAY_QUEUE_DEPTH: usize = 64 * 16;

        // Calculate total client threads for queue sizing
        let client_threads = config.client().map(|c| c.poolsize() * c.concurrency()).unwrap_or(1)
            * config.target().endpoints().len();
        let queue_depth = client_threads * REPLAY_QUEUE_DEPTH;

        // Create channel for replay work items
        let (replay_sender, replay_receiver) = bounded(queue_depth);

        // Create a Generator for replay mode that receives from the channel
        let replay_generator = Generator::new_for_replay(&config, replay_receiver);

        // spawn the admin thread (no ratelimiter in replay mode)
        control_runtime.spawn(admin::http(config.clone(), None));

        // launch metrics file output
        control_runtime.spawn(output::metrics(config.clone()));

        // begin cli output
        control_runtime.spawn(output::log(config.clone(), true));

        debug!("Launching replay clients");
        let replay_runtime = clients::cache::launch(&config, replay_generator);

        debug!("Launching replay workload");
        let replay_workload_runtime = launch_replay_workload(config.clone(), replay_sender);

        debug!("Waiting for test to complete");
        while RUNNING.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
        }

        if let Some(replay_runtime) = replay_runtime {
            replay_runtime.shutdown_timeout(std::time::Duration::from_millis(100));
        }
        replay_workload_runtime.shutdown_timeout(std::time::Duration::from_millis(100));
        info!("Shutdown replay workload and runtime");

        // delay before exiting
        while WAIT.load(Ordering::Relaxed) > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        return;
    }

    // otherwise continue onwards with the normal workload generator

    info!("Initializing workload generator");
    let workload_generator = Generator::new(&config);

    let workload_ratelimit = workload_generator.ratelimiter();

    let workload_components = workload_generator.components().to_owned();

    // spawn the admin thread
    control_runtime.spawn(admin::http(config.clone(), workload_ratelimit.clone()));

    // launch metrics file output
    control_runtime.spawn(output::metrics(config.clone()));

    // begin cli output
    control_runtime.spawn(output::log(config.clone(), false));

    debug!("Starting clients");
    // start client(s) - work generation now happens inside client tasks
    let client_runtime = clients::cache::launch(&config, workload_generator.clone());

    // start store client(s)
    let store_runtime = clients::store::launch(&config, workload_generator.clone());

    // start leaderboard client(s)
    let leaderboard_runtime = clients::leaderboard::launch(&config, workload_generator.clone());

    // start OLTP clients
    let oltp_runtime = clients::oltp::launch(&config, workload_generator.clone());

    // start publisher(s) and subscriber(s)
    let mut pubsub_runtimes =
        clients::pubsub::launch(&config, workload_generator, &workload_components);

    // start ratelimit controller thread if a dynamic ratelimit is configured
    {
        if let Some(mut ratelimit_controller) = Ratelimit::new(&config) {
            control_runtime.spawn(async move {
                while RUNNING.load(Ordering::Relaxed) {
                    let _ = admin::handlers::update_ratelimit(
                        ratelimit_controller.next_ratelimit(),
                        workload_ratelimit.clone(),
                    )
                    .await;
                    // delay until next step function
                    sleep(ratelimit_controller.interval()).await;
                }
            });
        }
    }

    debug!("Waiting for test to complete");
    while RUNNING.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
    }

    // shutdown thread pools

    if let Some(client_runtime) = client_runtime {
        client_runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    }

    if let Some(store_runtime) = store_runtime {
        store_runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    }

    if let Some(leaderboard_runtime) = leaderboard_runtime {
        leaderboard_runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    }

    if let Some(rt) = oltp_runtime {
        rt.shutdown_timeout(std::time::Duration::from_millis(100));
    }

    pubsub_runtimes.shutdown_timeout(std::time::Duration::from_millis(100));

    // delay before exiting

    while WAIT.load(Ordering::Relaxed) > 0 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    std::thread::sleep(std::time::Duration::from_millis(100));
}
