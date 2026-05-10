# ip-checker

a simple IP checker website, with customizable IP address classification

## config

this service is configured by a json configuration file.
you can pass in the config file either via the `IP_CHECKER_CONFIG` envvar (config content) or the `IP_CHECKER_CONFIG_FILE` envvar (path to config file).

refer to [`config.json.example`](./config.json.example) and [`src/config.rs`](./src/config.rs) for details on possible config fields
