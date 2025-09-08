use super::*;
use crate::clients::*;

use ::momento::cache::configurations::LowLatency;
use ::momento::*;

mod commands;

use commands::*;

pub mod http;

/// Launch tasks with one channel per task as gRPC is mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    work_receiver: Receiver<ClientWorkItemKind<ClientRequest>>,
) {
    debug!("launching momento protocol tasks");

    // Check for API key once before spawning tasks
    if std::env::var("MOMENTO_API_KEY").is_err() {
        eprintln!("environment variable `MOMENTO_API_KEY` is not set");
        std::process::exit(1);
    }

    let poolsize = config.client().unwrap().poolsize();
    let concurrency = config.client().unwrap().concurrency();

    // Create a dedicated runtime for connection establishment
    // This prevents blocking the main runtime and allows better parallelization
    let connection_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(poolsize.min(16)) // Cap at 16 threads to avoid excessive resource usage
        .enable_all()
        .build()
        .expect("failed to create connection runtime");

    // Create all clients in parallel using the dedicated runtime
    let clients = connection_runtime.block_on(async {
        let mut handles = Vec::with_capacity(poolsize);

        // Spawn all client creation tasks in parallel
        for _ in 0..poolsize {
            let handle = tokio::spawn(async move {
                // Initialize the Momento cache client asynchronously
                let credential_provider =
                    match CredentialProvider::from_env_var("MOMENTO_API_KEY".to_string()) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("MOMENTO_API_KEY key should be valid: {e}");
                            std::process::exit(1);
                        }
                    };

                let client = match CacheClient::builder()
                    .default_ttl(Duration::from_secs(900))
                    .configuration(LowLatency::v1())
                    .credential_provider(credential_provider)
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("could not create cache client: {}", e);
                        std::process::exit(1);
                    }
                };

                CONNECT.increment();
                CONNECT_CURR.increment();

                client
            });

            handles.push(handle);
        }

        // Wait for all clients to complete
        let mut clients = Vec::with_capacity(poolsize);
        for handle in handles {
            clients.push(handle.await.unwrap());
        }
        clients
    });

    // Shutdown the connection runtime as it's no longer needed
    connection_runtime.shutdown_background();

    // Spawn worker tasks on the main runtime
    for client in clients {
        // Create one task per channel for this client
        for _ in 0..concurrency {
            runtime.spawn(task(config.clone(), client.clone(), work_receiver.clone()));
        }
    }
}

async fn task(
    config: Config,
    mut client: CacheClient,
    work_receiver: Receiver<ClientWorkItemKind<ClientRequest>>,
) -> Result<()> {
    let cache_name = config.target().cache_name().unwrap_or_else(|| {
        eprintln!("cache name is not specified in the `target` section");
        std::process::exit(1);
    });

    while RUNNING.load(Ordering::Relaxed) {
        let work_item = work_receiver
            .recv()
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "channel closed"))?;

        REQUEST.increment();
        let start = Instant::now();
        let result = match work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                /*
                 * KEY-VALUE
                 */
                ClientRequest::Get(r) => get(&mut client, &config, cache_name, r).await,
                ClientRequest::Set(r) => set(&mut client, &config, cache_name, r).await,
                ClientRequest::Delete(r) => delete(&mut client, &config, cache_name, r).await,

                /*
                 * HASHES (DICTIONARIES)
                 */
                ClientRequest::HashDelete(r) => {
                    hash_delete(&mut client, &config, cache_name, r).await
                }
                ClientRequest::HashGet(r) => hash_get(&mut client, &config, cache_name, r).await,
                ClientRequest::HashGetAll(r) => {
                    hash_get_all(&mut client, &config, cache_name, r).await
                }
                ClientRequest::HashIncrement(r) => {
                    hash_increment(&mut client, &config, cache_name, r).await
                }
                ClientRequest::HashSet(r) => hash_set(&mut client, &config, cache_name, r).await,

                /*
                 * SETS
                 */
                ClientRequest::SetAdd(r) => set_add(&mut client, &config, cache_name, r).await,
                ClientRequest::SetMembers(r) => {
                    set_members(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SetRemove(r) => {
                    set_remove(&mut client, &config, cache_name, r).await
                }

                /*
                 * LISTS
                 */
                ClientRequest::ListPushFront(r) => {
                    list_push_front(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListPushBack(r) => {
                    list_push_back(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListFetch(r) => {
                    list_fetch(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListLength(r) => {
                    list_length(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListPopFront(r) => {
                    list_pop_front(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListPopBack(r) => {
                    list_pop_back(&mut client, &config, cache_name, r).await
                }
                ClientRequest::ListRemove(r) => {
                    list_remove(&mut client, &config, cache_name, r).await
                }

                /*
                 * SORTED SETS
                 */
                ClientRequest::SortedSetAdd(r) => {
                    sorted_set_add(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SortedSetIncrement(r) => {
                    sorted_set_increment(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SortedSetRange(r) => {
                    sorted_set_range(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SortedSetRank(r) => {
                    sorted_set_rank(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SortedSetRemove(r) => {
                    sorted_set_remove(&mut client, &config, cache_name, r).await
                }
                ClientRequest::SortedSetScore(r) => {
                    sorted_set_score(&mut client, &config, cache_name, r).await
                }

                /*
                 * UNSUPPORTED
                 */
                _ => {
                    REQUEST_UNSUPPORTED.increment();
                    continue;
                }
            },
            ClientWorkItemKind::Reconnect => {
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
            Err(ResponseError::Exception) => {
                RESPONSE_EX.increment();
            }
            Err(ResponseError::Timeout) => {
                RESPONSE_TIMEOUT.increment();
            }
            Err(ResponseError::Ratelimited) => {
                RESPONSE_RATELIMITED.increment();
            }
            Err(ResponseError::BackendTimeout) => {
                RESPONSE_BACKEND_TIMEOUT.increment();
            }
        }
    }

    Ok(())
}
