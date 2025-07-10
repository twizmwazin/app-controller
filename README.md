App Controller
================

App Controller is a simple REST API server that allow managing applications on
a kubernetes cluster. The purpose is to provide a simple interface for managing
container based applications, especially graphical ones.

# Options

App controller currently does not accept any command line parameters. It does
read from a few environment variables:
- `APP_CONTROLLER_BACKEND`: `kubernetes` (default) or `null`. With the
    kubernetes backend, all functionality is supported. With the null backend,
    all endpoints should respond without erroring, but do not execute any
    functionality. This is intended for testing and reading the integrated
    documentation.
- `RUST_LOG`: Controls the log level. Common values are `error`, `warn`, `info`,
    `debug`, `trace`, or `off`. See [here](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)
    for more information.
