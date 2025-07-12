App Controller
================

App Controller is a REST API server that allow managing remotely-accessible
graphical applications on a kubernetes cluster. The purpose of this is to allow
easily integrating native desktop applications into web user interfaces.

# Installation

## From source

App Controller is a Rust project that tries to follow language standards as much
as possible. To run, clone with git, and cargo build:

```sh
git clone https://github.com/twizmwazin/app-controller.git
cd app-controller
cargo build --release
```

## Container

App Controller is published automatically to ghcr.io on each commit to `main`.
Run the container like this:

```sh
docker run -p 3000:3000 ghcr.io/twizmwazin/app-controller/app-controller
```

# API Documentation

Documentation is served automatically at `/doc` on App Controller. This can be
easily viewed on a developer machine by running App Controller in a container
and using a web browser. First, run the container:

```sh
docker run --rm -e APP_CONTROLLER_BACKEND=null -p 3000:3000 ghcr.io/twizmwazin/app-controller/app-controller
```

Then, open http://localhost:3000 in a web browser.

# Configuration

App Controller currently does not accept any command line parameters. It does
read from a few environment variables:
- `APP_CONTROLLER_BACKEND`: `kubernetes` (default) or `null`. With the
    kubernetes backend, all functionality is supported. With the null backend,
    all endpoints should respond without erroring, but do not execute any
    functionality. This is intended for testing and reading the integrated
    documentation.
- `RUST_LOG`: Controls the log level. Common values are `error`, `warn`, `info`,
    `debug`, `trace`, or `off`. See [here](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)
    for more information.

There are also backend-specific options:
- Kubernetes: `KUBE_CONFIG`: This file points to the Kubernetes client
    configuration to be used by App Controller. Default is `~/.kube/config`.

# Data Management

App Controller stores as little data as possible, all in the container backend.
For Kubernetes, this is a series of labels, annotations, and config objects. See
[./src/backend/kubernetes.rs](./src/backend/kubernetes.rs) for details.
