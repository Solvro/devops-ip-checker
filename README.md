# ip-checker

a simple IP checker website, with customizable IP address classification

## config

this service is configured by a json configuration file.
you can pass in the config file via one of the following envvars:

- `IP_CHECKER_CONFIG`: config content
- `IP_CHECKER_CONFIG_B64`: config content as base64
- `IP_CHECKER_CONFIG_FILE`: path to config file

refer to [`config.example.json`](./config.example.json) and [`src/config.rs`](./src/config.rs) for details on possible config fields
