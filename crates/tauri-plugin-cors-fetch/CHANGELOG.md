# v2.1.0

- Fix: Exclude Tauri IPC requests from the request hook.

# v2.0.0

- New: Hook `fetch` requests and redirect them to [tauri-plugin-http](https://crates.io/crates/tauri-plugin-http).

# v1.0.0

- New: Hook `fetch` requests and redirect them to `x-http` and `x-https` custom protocols.
