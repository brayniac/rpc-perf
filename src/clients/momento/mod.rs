use super::*;
use ::momento::response::*;
use ::momento::*;
use std::borrow::Borrow;
use std::collections::HashMap;

mod commands;

use commands::*;

/// Launch tasks with one channel per task as gRPC is mux-enabled.
pub fn launch_tasks(runtime: &mut Runtime, config: Config, work_receiver: Receiver<WorkItem>) {
    debug!("launching momento protocol tasks");

    for _ in 0..config.client().unwrap().poolsize() {
        let client = {
            let _guard = runtime.enter();

            // initialize the Momento cache client
            if std::env::var("MOMENTO_AUTHENTICATION").is_err() {
                eprintln!("environment variable `MOMENTO_AUTHENTICATION` is not set");
                std::process::exit(1);
            }
            let auth_token = std::env::var("MOMENTO_AUTHENTICATION")
                .expect("MOMENTO_AUTHENTICATION must be set");
            match SimpleCacheClientBuilder::new(auth_token, std::time::Duration::from_secs(600)) {
                Ok(c) => c.build(),
                Err(e) => {
                    eprintln!("could not create cache client: {}", e);
                    std::process::exit(1);
                }
            }
        };

        CONNECT.increment();
        CONNECT_CURR.add(1);

        // create one task per channel
        for _ in 0..config.client().unwrap().concurrency() {
            runtime.spawn(task(config.clone(), client.clone(), work_receiver.clone()));
        }
    }
}

async fn task(
    config: Config,
    // cache_name: String,
    mut client: SimpleCacheClient,
    work_receiver: Receiver<WorkItem>,
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
            WorkItem::Request { request, .. } => match request {
                ClientRequest::Get(r) => get(&mut client, &config, cache_name, r).await,
                ClientRequest::Set(r) => set(&mut client, &config, cache_name, r).await,
                ClientRequest::Delete(r) => delete(&mut client, &config, cache_name, r).await,

                /*
                 * HASHES (DICTIONARIES)
                 */
                ClientRequest::HashDelete(r) => hash_delete(&mut client, &config, cache_name, r).await,
                ClientRequest::HashGet(r) => hash_get(&mut client, &config, cache_name, r).await,
                ClientRequest::HashGetAll(r) => hash_get_all(&mut client, &config, cache_name, r).await,
                ClientRequest::HashIncrement { key, field, amount } => {
                    HASH_INCR.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.dictionary_increment(
                            cache_name,
                            &*key,
                            &*field,
                            amount,
                            CollectionTtl::new(None, false),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(r)) => {
                            HASH_INCR_OK.increment();
                            #[allow(clippy::if_same_then_else)]
                            if r.value == amount {
                                RESPONSE_MISS.increment();
                                HASH_INCR_MISS.increment();
                            } else {
                                RESPONSE_HIT.increment();
                                HASH_INCR_HIT.increment();
                            }
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            HASH_INCR_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            HASH_INCR_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::HashSet { key, data } => {
                    HASH_SET.increment();
                    let data: HashMap<Vec<u8>, Vec<u8>> =
                        data.iter().map(|(k, v)| (k.to_vec(), v.to_vec())).collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.dictionary_set(
                            cache_name,
                            &*key,
                            data,
                            CollectionTtl::new(None, false),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            HASH_SET_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            HASH_SET_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            HASH_SET_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }

                /*
                 * SETS
                 */
                ClientRequest::SetAdd { key, members } => {
                    SET_ADD.increment();
                    let members: Vec<&[u8]> = members.iter().map(|v| v.borrow()).collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.set_add_elements(
                            cache_name,
                            &*key,
                            members,
                            CollectionTtl::new(None, false),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SET_ADD_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SET_ADD_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SET_ADD_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SetMembers { key } => {
                    SET_MEMBERS.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.set_fetch(cache_name, &*key),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SET_MEMBERS_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SET_MEMBERS_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SET_MEMBERS_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SetRemove { key, members } => {
                    SET_REMOVE.increment();
                    let members: Vec<&[u8]> = members.iter().map(|v| v.borrow()).collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.set_remove_elements(cache_name, &*key, members),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SET_REMOVE_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SET_REMOVE_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SET_REMOVE_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }

                /*
                 * LISTS
                 */
                ClientRequest::ListPushFront {
                    key,
                    elements,
                    truncate,
                } => {
                    LIST_PUSH_FRONT.increment();
                    let result = if elements.len() == 1 {
                        match timeout(
                            config.client().unwrap().request_timeout(),
                            client.list_push_front(
                                cache_name,
                                &*key,
                                &*elements[0],
                                truncate,
                                CollectionTtl::new(None, false),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.into()),
                            Err(_) => Err(ResponseError::Timeout),
                        }
                    } else {
                        // note: we need to reverse because the semantics of list
                        // concat do not match the redis push semantics
                        let elements: Vec<&[u8]> =
                            elements.iter().map(|v| v.borrow()).rev().collect();
                        match timeout(
                            config.client().unwrap().request_timeout(),
                            client.list_concat_front(
                                cache_name,
                                &*key,
                                elements,
                                truncate,
                                CollectionTtl::new(None, false),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.into()),
                            Err(_) => Err(ResponseError::Timeout),
                        }
                    };
                    match result {
                        Ok(_) => {
                            LIST_PUSH_FRONT_OK.increment();
                        }
                        Err(ResponseError::Timeout) => {
                            LIST_PUSH_FRONT_TIMEOUT.increment();
                        }
                        Err(_) => {
                            LIST_PUSH_FRONT_EX.increment();
                        }
                    }

                    result
                }
                ClientRequest::ListPushBack {
                    key,
                    elements,
                    truncate,
                } => {
                    LIST_PUSH_BACK.increment();
                    let result = if elements.len() == 1 {
                        match timeout(
                            config.client().unwrap().request_timeout(),
                            client.list_push_back(
                                cache_name,
                                &*key,
                                &*elements[0],
                                truncate,
                                CollectionTtl::new(None, false),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.into()),
                            Err(_) => Err(ResponseError::Timeout),
                        }
                    } else {
                        // note: we need to reverse because the semantics of list
                        // concat do not match the redis push semantics
                        let elements: Vec<&[u8]> =
                            elements.iter().map(|v| v.borrow()).rev().collect();
                        match timeout(
                            config.client().unwrap().request_timeout(),
                            client.list_concat_back(
                                cache_name,
                                &*key,
                                elements,
                                truncate,
                                CollectionTtl::new(None, false),
                            ),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.into()),
                            Err(_) => Err(ResponseError::Timeout),
                        }
                    };
                    match result {
                        Ok(_) => {
                            LIST_PUSH_BACK_OK.increment();
                        }
                        Err(ResponseError::Timeout) => {
                            LIST_PUSH_BACK_TIMEOUT.increment();
                        }
                        Err(_) => {
                            LIST_PUSH_BACK_EX.increment();
                        }
                    }

                    result
                }
                ClientRequest::ListFetch { key } => {
                    LIST_FETCH.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.list_fetch(cache_name, &*key),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            LIST_FETCH_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            LIST_FETCH_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            LIST_FETCH_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::ListLength { key } => {
                    LIST_LENGTH.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.list_fetch(cache_name, &*key),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            LIST_LENGTH_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            LIST_LENGTH_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            LIST_LENGTH_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::ListPopFront { key } => {
                    LIST_POP_FRONT.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.list_pop_front(cache_name, &*key),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            LIST_POP_FRONT_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            LIST_POP_FRONT_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            LIST_POP_FRONT_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::ListPopBack { key } => {
                    LIST_POP_BACK.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.list_pop_back(cache_name, &*key),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            LIST_POP_BACK_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            LIST_POP_BACK_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            LIST_POP_BACK_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }

                /*
                 * SORTED SETS
                 */
                ClientRequest::SortedSetAdd { key, members } => {
                    SORTED_SET_ADD.increment();
                    let members: Vec<sorted_set::SortedSetElement> = members
                        .iter()
                        .map(|(value, score)| sorted_set::SortedSetElement {
                            value: value.to_vec(),
                            score: *score,
                        })
                        .collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_put(
                            cache_name,
                            &*key,
                            members,
                            CollectionTtl::new(None, false),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_ADD_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_ADD_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_ADD_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SortedSetMembers { key } => {
                    SORTED_SET_MEMBERS.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_fetch(
                            cache_name,
                            &*key,
                            momento::sorted_set::Order::Ascending,
                            None,
                            None,
                        ),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_MEMBERS_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_MEMBERS_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_MEMBERS_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SortedSetIncrement {
                    key,
                    member,
                    amount,
                } => {
                    SORTED_SET_INCR.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_increment(
                            cache_name,
                            &*key,
                            &*member,
                            amount,
                            CollectionTtl::new(None, false),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_INCR_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_INCR_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_INCR_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SortedSetRank { key, member } => {
                    SORTED_SET_RANK.increment();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_get_rank(cache_name, &*key, &*member),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_RANK_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_RANK_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_RANK_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SortedSetRemove { key, members } => {
                    SORTED_SET_REMOVE.increment();
                    let members: Vec<&[u8]> = members.iter().map(|v| v.borrow()).collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_remove(cache_name, &*key, members),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_REMOVE_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_REMOVE_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_REMOVE_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                ClientRequest::SortedSetScore { key, members } => {
                    SORTED_SET_SCORE.increment();
                    let members: Vec<&[u8]> = members.iter().map(|v| v.borrow()).collect();
                    match timeout(
                        config.client().unwrap().request_timeout(),
                        client.sorted_set_get_score(cache_name, &*key, members),
                    )
                    .await
                    {
                        Ok(Ok(_)) => {
                            SORTED_SET_SCORE_OK.increment();
                            Ok(())
                        }
                        Ok(Err(e)) => {
                            SORTED_SET_SCORE_EX.increment();
                            Err(e.into())
                        }
                        Err(_) => {
                            SORTED_SET_SCORE_TIMEOUT.increment();
                            Err(ResponseError::Timeout)
                        }
                    }
                }
                _ => {
                    REQUEST_UNSUPPORTED.increment();
                    continue;
                }
            },
            WorkItem::Reconnect => {
                continue;
            }
        };

        REQUEST_OK.increment();

        let stop = Instant::now();

        match result {
            Ok(_) => {
                RESPONSE_OK.increment();

                let latency = stop.duration_since(start).as_nanos();

                REQUEST_LATENCY.increment(start, latency, 1);
                RESPONSE_LATENCY.increment(stop, latency, 1);
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