# sd-client

Rust client for the Spacedrive daemon, used by `apps/cli` and native Rust apps.

- `CoreClient` — typed `query`/`action` calls for `Wire`-registered ops, raw
  daemon requests, and event/log subscriptions (`EventStream`, `LogStream`)
- `SubscriptionBroker` — pools event subscriptions: one daemon connection per
  distinct `(event_types, filter)` signature, fanned out to any number of
  `BrokerSubscription` receivers, with linger on last drop and reconnect with
  capped exponential backoff (duplicates possible across reconnects)
- `LibraryContext` — current library selection, persisted to a JSON file and
  observable via a watch channel; `query`/`action` wrappers inject the id
- `daemon_socket_addr` — maps an instance name to the daemon's loopback address
- `ensure_daemon` / `is_daemon_running` — ping the daemon and spawn it in the
  background when it is not running

```rust
use sd_client::{daemon_socket_addr, CoreClient, LibraryContext, SubscriptionBroker};

let addr = daemon_socket_addr(None).to_string();
let client = CoreClient::new(addr.clone());
let status: CoreStatus = client.query(&StatusInput {}, None).await?;

// Many views, one daemon connection per distinct filter.
let broker = SubscriptionBroker::new(addr);
let mut events = broker.subscribe(vec!["ResourceChanged".into()], None);
while let Some(event) = events.recv().await { /* invalidate caches */ }

// Library-scoped calls without threading the id everywhere.
let library = LibraryContext::load(client, state_dir.join("library.json")).await?;
library.set_current(Some(library_id)).await?;
let config: LibrarySettingsOutput = library.query(&GetLibraryConfigQueryInput).await?;
```
