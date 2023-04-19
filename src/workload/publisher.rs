// SPDX-License-Identifier: (Apache-2.0)
// Copyright Authors of rpc-perf

use std::sync::Arc;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum PublisherWorkItem {
    Publish {
        topic: Arc<String>,
        message: Vec<u8>,
    },
}