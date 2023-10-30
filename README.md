# rpc-perf

rpc-perf is a tool for measuring the performance of network services. While it
has historically focused on caching services, the support is expanding to cover
HTTP and PubSub.

[![License: Apache-2.0][license-badge-apache]][license-url-apache]
[![License: MIT][license-badge-mit]][license-url-mit]
[![Build Status: CI][ci-build-badge]][ci-build-url]

# Content
* [Getting started](#getting-started)
* [Building from source](#building-from-source)
* [Contributing](#contributing)

# Getting started

Follow the [build instructions](#building-from-source) to build rpc-perf from
this repository and take a look at the example configurations in the `configs`
folder. There you will find some examples for each of the supported protocols.
The examples provide a starting point and may need some changes to produce a
representative workload for your testing.

# Building from source

To build rpc-perf from source, you will need a current Rust toolchain. If you
don't have one, you can use [rustup](https://rustup.rs) or follow the
instructions on [rust-lang.org](https://rust-lang.org).

Now that you have a Rust toolchain installed, you can clone and build the
project using the `cargo` command.

```bash
git clone https://github.com/iopsystems/rpc-perf
cd rpc-perf
cargo build --release
```

This will produce a binary at `target/release/rpc-perf` which you may copy to
a more convenient location. Check out the [getting started](#getting-started)
for more information on how to use rpc-perf

# Contributing

If you want to submit a patch, please follow these steps:

1. create a new issue
2. fork on github & clone your fork
3. create a feature branch on your fork
4. push your feature branch
5. create a pull request linked to the issue

[ci-build-badge]: https://img.shields.io/github/actions/workflow/status/iopsystems/rpc-perf/cargo.yml?branch=main
[ci-build-url]: https://github.com/iopsystems/rpc-perf/actions/workflows/cargo.yml?query=branch%3Amaster+event%3Apush
[license-badge-apache]: https://img.shields.io/badge/license-Apache%202.0-blue.svg
[license-badge-mit]: https://img.shields.io/badge/license-MIT-blue.svg
[license-url-apache]: https://github.com/iopsystems/rpc-perf/blob/master/LICENSE-APACHE
[license-url-mit]: https://github.com/iopsystems/rpc-perf/blob/master/LICENSE-MIT
