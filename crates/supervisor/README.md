# sd-supervisor

Machine-scoped process supervision for the daemon. A host has one process
table, so it gets one supervisor and one owner of the port ledger.

The crate is the kernel only — a service state machine, health probing,
restart policy with backoff, per-service log capture, and a persistent port
ledger. It decides nothing about *which* services should run; callers register
services and the daemon arbitrates. Core surfaces it as the `processes` ops
domain, which puts it on the API, the generated TypeScript and Swift types,
and `sd op` in one step.

| Module | Role |
|---|---|
| `supervisor.rs` | service state machine, adoption, restart policy |
| `ledger.rs` | port leases, persisted per machine |
| `spec.rs` | `SpawnSpec` and the service kinds |
| `probe.rs` | health probing |
| `logs.rs` | per-service log capture |
| `exec.rs`, `timing.rs` | process exec and the backoff schedule |
| `compose.rs`, `container.rs` | the compose service kind, driven through the CLI |

Behaviour worth knowing: startup adopts before it owns, so anything already
alive is left exactly as it is and never restarted on the way out. Ownership is
tracked as `owned | adopted | compose | external`. Health runs on a 30s
interval; three consecutive failures retire an adopted service and respawn it
as owned. Exits under 10s count toward a cap of five, with backoff doubling
from 1s to a 60s ceiling and a 10s stop grace.

Supervision lives in the daemon rather than in a client because it has to
outlive the shell — the daemon is the machine's always-on process.

The planning document this crate was extracted from is not included here.
