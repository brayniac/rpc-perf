use crate::clients::*;
use crate::net::Connector;
use crate::workload::*;
use crate::*;
use protocol_ping::{Compose, Parse, Request, Response};
use rand::{RngCore, SeedableRng};
use rand_xoshiro::{Seed512, Xoshiro512PlusPlus};
use session::{Buf, BufMut, Buffer};
use std::borrow::{Borrow, BorrowMut};
use std::io::{ErrorKind, Result};
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;
use tokio::time::timeout;

/// Launch tasks with one connection per task as ping protocol is not mux-enabled.
pub fn launch_tasks(
    runtime: &mut Runtime,
    config: Config,
    generator: Generator,
    rng: &mut Xoshiro512PlusPlus,
) {
    debug!("launching ping ascii protocol tasks");

    // create one task per "connection"
    // note: these may be channels instead of connections for multiplexed protocols
    for _ in 0..config.client().unwrap().poolsize() {
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

// a task for ping servers (eg: Pelikan Pingserver)
#[allow(clippy::slow_vector_initialization)]
async fn task(
    endpoint: String,
    config: Config,
    generator: Generator,
    seed: Seed512,
) -> Result<()> {
    let connector = Connector::new(&config)?;

    // this unwrap will succeed because we wouldn't be creating these tasks if
    // there wasn't a client config.
    let client_config = config.client().unwrap();

    let mut stream = None;
    let parser = protocol_ping::ResponseParser::new();
    let mut read_buffer = Buffer::new(client_config.read_buffer_size());
    let mut write_buffer = Buffer::new(client_config.write_buffer_size());
    let mut rng = Xoshiro512PlusPlus::from_seed(seed);

    while RUNNING.load(Ordering::Relaxed) {
        if stream.is_none() {
            CONNECT.increment();
            stream = match timeout(
                client_config.connect_timeout(),
                connector.connect(&endpoint),
            )
            .await
            {
                Ok(Ok(s)) => {
                    CONNECT_OK.increment();
                    CONNECT_CURR.increment();
                    Some(s)
                }
                Ok(Err(_)) => {
                    CONNECT_EX.increment();
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(_) => {
                    CONNECT_TIMEOUT.increment();
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }
        }

        let mut s = stream.take().unwrap();

        // Wait for ratelimiter and generate request locally
        generator.wait();

        let work_item = match generator.generate_client_request(&mut rng) {
            Some(item) => item,
            None => {
                stream = Some(s);
                continue;
            }
        };

        REQUEST.increment();

        // compose request into buffer
        match &work_item {
            ClientWorkItemKind::Request { request, .. } => match request {
                ClientRequest::Ping(_) => {
                    Request::Ping.compose(&mut write_buffer);
                }
                _ => {
                    REQUEST_UNSUPPORTED.increment();
                    stream = Some(s);
                    continue;
                }
            },
            ClientWorkItemKind::Reconnect => {
                REQUEST_RECONNECT.increment();
                continue;
            }
        }

        REQUEST_OK.increment();

        // send request
        let start = Instant::now();
        s.write_all(write_buffer.borrow()).await?;
        write_buffer.clear();

        // read until response or timeout
        let mut remaining_time = client_config.request_timeout().as_nanos() as u64;
        let response = loop {
            match timeout(
                Duration::from_millis(remaining_time / 1000000),
                s.read(read_buffer.borrow_mut()),
            )
            .await
            {
                Ok(Ok(n)) => {
                    unsafe {
                        read_buffer.advance_mut(n);
                    }
                    match parser.parse(read_buffer.borrow()) {
                        Ok(resp) => {
                            let consumed = resp.consumed();
                            let resp = resp.into_inner();

                            read_buffer.advance(consumed);

                            break Ok(resp);
                        }
                        Err(e) => match e.kind() {
                            ErrorKind::WouldBlock => {
                                let elapsed = start.elapsed().as_nanos() as u64;
                                remaining_time = remaining_time.saturating_sub(elapsed);
                                if remaining_time == 0 {
                                    break Err(ResponseError::Timeout);
                                }
                            }
                            _ => {
                                break Err(ResponseError::Exception);
                            }
                        },
                    }
                }
                Ok(Err(_)) => {
                    break Err(ResponseError::Exception);
                }
                Err(_) => {
                    break Err(ResponseError::Timeout);
                }
            }
        };

        let stop = Instant::now();

        match response {
            Ok(response) => {
                // validate response
                match work_item {
                    ClientWorkItemKind::Request { request, .. } => match request {
                        ClientRequest::Ping(_) => match response {
                            Response::Pong => {
                                PING_OK.increment();
                            }
                        },
                        _ => {
                            error!("unexpected request");
                            unimplemented!();
                        }
                    },
                    _ => {
                        error!("unexpected work item");
                        unimplemented!();
                    }
                }

                RESPONSE_OK.increment();

                let latency = stop.duration_since(start).as_nanos() as u64;

                let _ = RESPONSE_LATENCY.increment(latency);

                // Check if we should reconnect
                if generator.should_reconnect() {
                    CONNECT_CURR.decrement();
                } else {
                    // preserve the connection for reuse
                    stream = Some(s);
                }
            }
            Err(ResponseError::Exception) => {
                // record execption
                match work_item {
                    ClientWorkItemKind::Request { request, .. } => match request {
                        ClientRequest::Ping(_) => {
                            PING_EX.increment();
                        }
                        _ => {
                            error!("unexpected request");
                            unimplemented!();
                        }
                    },
                    _ => {
                        error!("unexpected work item");
                        unimplemented!();
                    }
                }

                CONNECT_CURR.sub(1);
            }
            Err(ResponseError::Timeout) => {
                RESPONSE_TIMEOUT.increment();
                CONNECT_CURR.sub(1);
            }
            Err(ResponseError::Ratelimited) | Err(ResponseError::BackendTimeout) => {
                unimplemented!();
            }
        }
    }

    Ok(())
}
