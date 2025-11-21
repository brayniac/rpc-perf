use crate::clients::cache::*;
use crate::clients::common::Queue;
use crate::clients::*;
use crate::workload::Generator;

use ::redis::aio::MultiplexedConnection;
use ::redis::AsyncCommands;

use std::borrow::Borrow;
use std::net::SocketAddr;

mod commands;

use commands::*;

/// Launch tasks with one connection per task as RESP protocol is not mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    generator: Generator,
    rng: &mut Xoshiro512PlusPlus,
) {
    debug!("launching resp protocol tasks");

    // create one task per "connection"
    // note: these may be channels instead of connections for multiplexed protocols
    for _ in 0..config.client().unwrap().poolsize() {
        for endpoint in config.target().endpoints() {
            // for each endpoint there are poolsize # of pool managers, each
            // managed a single connection

            let queue = Queue::new(1);
            runtime.spawn(pool_manager(
                endpoint.clone(),
                config.clone(),
                queue.clone(),
            ));

            // one task for each concurrent session on the connection

            for _ in 0..config.client().unwrap().concurrency() {
                // Generate unique seed for this task
                let mut seed = [0u8; 64];
                rng.fill_bytes(&mut seed);

                runtime.spawn(task(
                    endpoint.clone(),
                    config.clone(),
                    queue.clone(),
                    generator.clone(),
                    Seed512(seed),
                ));
            }
        }
    }
}

pub async fn pool_manager(endpoint: String, _config: Config, queue: Queue<MultiplexedConnection>) {
    let mut client = None;

    let endpoint = if endpoint.parse::<SocketAddr>().is_ok() {
        format!("redis://{endpoint}")
    } else {
        endpoint
    };

    while RUNNING.load(Ordering::Relaxed) {
        if client.is_none() {
            match ::redis::Client::open(endpoint.clone()) {
                Ok(c) => {
                    client = Some(c);
                }
                Err(e) => {
                    error!("failed to create redis client: {e}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }
        }

        CONNECT.increment();

        match client
            .as_ref()
            .unwrap()
            .get_multiplexed_async_connection()
            .await
        {
            Ok(connection) => {
                CONNECT_OK.increment();
                CONNECT_CURR.increment();
                let _ = queue.send(connection).await;
            }
            Err(e) => {
                CONNECT_EX.increment();
                error!("failed to get redis connection: {e}");
                client = None;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

#[allow(dead_code)]
#[allow(clippy::slow_vector_initialization)]
async fn task(
    endpoint: String,
    config: Config,
    queue: Queue<MultiplexedConnection>,
    generator: Generator,
    seed: Seed512,
) -> Result<()> {
    trace!("launching resp task for endpoint: {endpoint}");

    let mut connection = None;
    let mut rng = Xoshiro512PlusPlus::from_seed(seed);

    while RUNNING.load(Ordering::Relaxed) {
        if connection.is_none() {
            if let Ok(c) = queue.recv().await {
                connection = Some(c);
            } else {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            continue;
        }

        let mut con = connection.take().unwrap();

        // Wait for ratelimiter and generate request locally
        // This ensures ratelimiter wait time is included in latency measurement
        generator.wait();

        let work_item = match generator.generate_client_request(&mut rng) {
            Some(item) => item,
            None => {
                // No keyspace component configured, skip
                connection = Some(con);
                continue;
            }
        };

        REQUEST.increment();
        let start = Instant::now();
        let mut latency_histograms = vec![&RESPONSE_LATENCY];
        let result = match work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                /*
                 * PING
                 */
                ClientRequest::Ping(r) => ping(&mut con, &config, r).await,

                /*
                 * KEY-VALUE
                 */
                ClientRequest::Add(r) => add(&mut con, &config, r).await,
                ClientRequest::Delete(r) => delete(&mut con, &config, r).await,
                ClientRequest::Get(r) => {
                    latency_histograms.push(&KVGET_RESPONSE_LATENCY);
                    get(&mut con, &config, r).await
                }
                ClientRequest::Replace(r) => replace(&mut con, &config, r).await,
                ClientRequest::Set(r) => {
                    latency_histograms.push(&KVSET_RESPONSE_LATENCY);
                    set(&mut con, &config, r).await
                }

                /*
                 * HASHES (DICTIONARIES)
                 */
                ClientRequest::HashDelete(r) => hash_delete(&mut con, &config, r).await,
                ClientRequest::HashExists(r) => hash_exists(&mut con, &config, r).await,
                ClientRequest::HashIncrement(r) => hash_increment(&mut con, &config, r).await,
                // transparently issues either a `hget` or `hmget`
                ClientRequest::HashGet(r) => hash_get(&mut con, &config, r).await,
                ClientRequest::HashGetAll(r) => hash_get_all(&mut con, &config, r).await,
                ClientRequest::HashSet(r) => hash_set(&mut con, &config, r).await,

                /*
                 * LISTS
                 */
                // To truncate, we must fuse an LTRIM at the end of the LPUSH
                ClientRequest::ListPushFront(r) => list_push_front(&mut con, &config, r).await,
                // To truncate, we must fuse an RTRIM at the end of the RPUSH
                ClientRequest::ListPushBack(r) => list_push_back(&mut con, &config, r).await,
                ClientRequest::ListFetch(r) => list_fetch(&mut con, &config, r).await,
                ClientRequest::ListLength(r) => list_length(&mut con, &config, r).await,
                ClientRequest::ListPopFront(r) => list_pop_front(&mut con, &config, r).await,
                ClientRequest::ListPopBack(r) => list_pop_back(&mut con, &config, r).await,

                /*
                 * SETS
                 */
                ClientRequest::SetAdd(r) => set_add(&mut con, &config, r).await,
                ClientRequest::SetMembers(r) => set_members(&mut con, &config, r).await,
                ClientRequest::SetRemove(r) => set_remove(&mut con, &config, r).await,

                /*
                 * SORTED SETS
                 */
                ClientRequest::SortedSetAdd(r) => sorted_set_add(&mut con, &config, r).await,
                ClientRequest::SortedSetRange(r) => sorted_set_range(&mut con, &config, r).await,
                ClientRequest::SortedSetIncrement(r) => {
                    sorted_set_increment(&mut con, &config, r).await
                }
                ClientRequest::SortedSetRemove(r) => sorted_set_remove(&mut con, &config, r).await,
                ClientRequest::SortedSetScore(r) => sorted_set_score(&mut con, &config, r).await,
                ClientRequest::SortedSetRank(r) => sorted_set_rank(&mut con, &config, r).await,

                /*
                 * UNSUPPORTED
                 */
                _ => {
                    REQUEST_UNSUPPORTED.increment();
                    connection = Some(con);
                    continue;
                }
            },
            ClientWorkItemKind::Reconnect => {
                CONNECT_CURR.decrement();
                continue;
            }
        };

        REQUEST_OK.increment();

        let stop = Instant::now();

        let latency_ns = stop.duration_since(start).as_nanos() as u64;

        match result {
            Ok(_) => {
                RESPONSE_OK.increment();
                for hist in latency_histograms {
                    let _ = hist.increment(latency_ns);
                }

                // Check if we should reconnect
                if generator.should_reconnect() {
                    CONNECT_CURR.decrement();
                } else {
                    connection = Some(con);
                }
            }
            Err(ResponseError::Exception) => {
                RESPONSE_EX.increment();
            }
            Err(ResponseError::Timeout) => {
                RESPONSE_TIMEOUT.increment();
            }
            Err(ResponseError::Ratelimited) => {
                RESPONSE_RATELIMITED.increment();
                connection = Some(con);
            }
            Err(ResponseError::BackendTimeout) => {
                RESPONSE_BACKEND_TIMEOUT.increment();
                connection = Some(con);
            }
        }
    }

    Ok(())
}
