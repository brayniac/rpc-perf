// Copyright 2023 IOP Systems, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

use ::momento::simple_cache_client::Fields;
use ::momento::simple_cache_client::SimpleCacheClient;
use ::momento::simple_cache_client::SimpleCacheClientBuilder;

use std::collections::HashMap;

pub fn launch_tasks(runtime: &mut Runtime, work_receiver: Receiver<WorkItem>) {
    let client_builder = {
        let _guard = runtime.enter();

        // initialize the Momento cache client
        if std::env::var("MOMENTO_AUTHENTICATION").is_err() {
            eprintln!("environment variable `MOMENTO_AUTHENTICATION` is not set");
            // let _ = log_drain.flush();
            std::process::exit(1);
        }
        let auth_token =
            std::env::var("MOMENTO_AUTHENTICATION").expect("MOMENTO_AUTHENTICATION must be set");
        let client_builder =
            match SimpleCacheClientBuilder::new(auth_token, NonZeroU64::new(600).unwrap()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("could not create cache client: {}", e);
                    // let _ = log_drain.flush();
                    std::process::exit(1);
                }
            };

        client_builder
    };

    // create one task per "connection"
    // note: these may be channels instead of connections for multiplexed protocols
    for _ in 0..CONNECTIONS {
        runtime.spawn(task(
            client_builder.clone().build(),
            work_receiver.clone(),
        ));
    }
}

async fn task(
    mut client: SimpleCacheClient,
    work_receiver: Receiver<WorkItem>,
) -> Result<()> {
    while RUNNING.load(Ordering::Relaxed) {
        let work_item = work_receiver
            .recv()
            .await
            .map_err(|_| Error::new(ErrorKind::Other, "channel closed"))?;

        let start = Instant::now();
        let result = match work_item {
            WorkItem::Get { key } => {
                GET.increment();
                timeout(
                    Duration::from_millis(200),
                    client.get("preview-cache", key.as_str()),
                )
                .await
                .map(|r| r.is_ok())
            },
            WorkItem::Set { key, value } => timeout(
                Duration::from_millis(200),
                client.set("preview-cache", key.as_str(), value.as_str(), None),
            )
            .await
            .map(|r| r.is_ok()),
            WorkItem::HashDelete { key, fields } => timeout(
                Duration::from_millis(200),
                client.dictionary_delete("preview-cache", key.as_str(), Fields::Some(fields.iter().map(|f| f.as_str()).collect())),
            )
            .await
            .map(|r| r.is_ok()),
            WorkItem::HashGet { key, field } => timeout(
                Duration::from_millis(200),
                client.dictionary_get("preview-cache", key.as_str(), vec![field.as_str()]),
            )
            .await
            .map(|r| r.is_ok()),
            WorkItem::HashMultiGet { key, fields } => timeout(
                Duration::from_millis(200),
                client.dictionary_get("preview-cache", key.as_str(), fields.iter().map(|f| f.as_str()).collect()),
            )
            .await
            .map(|r| r.is_ok()),
            WorkItem::HashSet { key, field, value } => timeout(
                Duration::from_millis(200),
                client.dictionary_set(
                    "preview-cache",
                    key.as_str(),
                    HashMap::from([(field.as_str(), value)]),
                    None,
                    false,
                ),
            )
            .await
            .map(|r| r.is_ok()),
            _ => {
                continue;
            }
        };

        let stop = Instant::now();

        if let Ok(ok) = result {
            if ok {
                RESPONSE_OK.increment();
                RESPONSE_LATENCY.increment(stop, stop.duration_since(start).as_nanos(), 1);
            } else {
                RESPONSE_EX.increment();
                RESPONSE_LATENCY.increment(stop, stop.duration_since(start).as_nanos(), 1);
            }
        } else {
            RESPONSE_TIMEOUT.increment();
        }
    }

    Ok(())
}