# Ceramic HTTP API Client

Ceramic [HTTP API](https://developers.ceramic.network/build/http/api/) client written in [rust](https://www.rust-lang.org/).
This library can either generate [serde](https://serde.rs/) compatible requests for use with any http client library, or make requests
against a Ceramic HTTP Api using [reqwest](https://docs.rs/reqwest/latest/reqwest/) when the `remote` feature flag is used (enabled by default).

Please see the [tests](./src/lib.rs) for more information on how to use the library.

