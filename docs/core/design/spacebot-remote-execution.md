# Spacebot Remote Execution Over Spacedrive

## Purpose

Define how a single Spacebot instance can operate across many user devices by using Spacedrive as the permission, transport, and execution layer.

This design assumes the normal long-term deployment model:

- one Spacebot instance
- one paired Spacedrive node owned by that Spacebot instance
- many user Spacedrive devices in the same library

Spacedrive becomes the system that decides what the agent can access, on which devices, and for which operations.

## Decision

Spacebot should never directly own the multi-device graph.

Instead:

- every Spacebot instance must be paired to a Spacedrive node
- that paired Spacedrive node is Spacebot's portal into the user's library
- all remote file access, shell access, future computer use, and other device-local execution go through Spacedrive
- Spacedrive is the source of truth for device identity, library membership, permissions, and remote dispatch

This means Spacedrive is the permission and execution layer. Spacebot is the agent runtime and scheduler.

## Why This Shape

Spacedrive already owns the hard distributed systems primitives:

- device identity
- pairing
- peer discovery
- library membership
- cross-device addressing
- file transfer
- library-scoped jobs

Spacebot already owns the hard agent-runtime primitives:

- channels, branches, workers
- memory and planning
- worker lifecycle
- tool orchestration
- model routing
- conversational UX

This split is the cleanest one:

- Spacedrive decides whether something may happen and where it runs
- Spacebot decides what work should be done and how to delegate it

## Deployment Model

### Local Install

- Spacedrive runs on the user's machine
- Spacebot runs as a subprocess of Spacedrive
- both connect to the same local library and device identity

### Hosted Install

- hosted infrastructure runs one Spacebot instance and one Spacedrive node together
- that Spacedrive node appears as a device in the user's library
- user devices pair with that library
- Spacebot uses its paired Spacedrive node to operate across the rest of the fleet

This avoids any central relay architecture beyond what Spacedrive already needs for peer connectivity.

## Product Model

The user-facing product model is:

- Spacedrive is installed on all user devices
- Spacebot is accessed through Spacedrive, not through a separate desktop app
- Spacebot Desktop is replaced by Spacedrive UI surfaces
- one Spacebot instance can act across the user's device fleet because Spacedrive provides the device graph and permission system

This is the long-term convergence point between the two products.

## Core Principle

Spacebot should know what devices exist and what it is allowed to do.

Spacebot should not be the final authority that enforces those decisions.

Spacedrive remains the enforcement layer.

That gives us:

- one security model
- one device graph
- one permission UX
- one audit surface
- one cross-device execution substrate

## Architecture

```text
User
  -> Spacedrive UI
    -> Spacebot runtime
      -> worker with execution_target
        -> Spacedrive client bound to paired Spacebot node
          -> permission + routing decision
            -> target Spacedrive device
              -> local execution on that device
```

### Responsibility Split

#### Spacebot owns

- chat and voice interaction
- planning and delegation
- spawning workers
- worker state and status
- memory
- model routing
- deciding which target device should perform a task

#### Spacedrive owns

- library authentication
- device graph
- path and subtree permissions
- capability permissions
- policy enforcement
- forwarding execution to the correct peer device
- auditing and eventing of remote operations

## Paired Node Model

Every Spacebot instance has exactly one paired Spacedrive node.

That node is Spacebot's home device inside the library.

It is responsible for:

- authenticating Spacebot to the library
- maintaining the current device graph
- resolving allowed targets
- forwarding remote operations to peer devices
- storing policy and audit metadata locally as part of the library context

This is the key simplification that avoids making Spacebot maintain direct relationships with many devices.

## Execution Model

### Current Spacebot behavior

Today, Spacebot workers execute shell and file tools on the machine where Spacebot itself runs.

### Target behavior

Workers keep the same conceptual tools:

- `shell`
- `file`
- future `computer_use`

But those tools become execution-target aware.

Each worker binds to exactly one `execution_target` when it is spawned.

Examples:

- local paired Spacebot device
- user's MacBook
- Windows workstation
- NAS device
- future mobile target for mobile-safe operations

The tool interface stays simple for the model. The transport changes under the hood.

## Recommended Spacebot Integration Shape

Add an `execution_target` abstraction to workers.

Recommended shape:

- keep current worker lifecycle unchanged
- keep current tool names unchanged
- swap local shell/file implementations for Spacedrive-backed proxy tools when target is remote

This means the model still thinks in terms of ordinary work:

- read files
- edit files
- run commands
- use the computer

But the actual execution happens through Spacedrive according to policy.

## Recommended Spacedrive Integration Shape

Add a new agent principal and remote execution protocol.

Spacedrive should support:

- identifying a Spacebot instance as a library-scoped principal
- resolving what devices and subtrees it may access
- forwarding typed operations to the correct device
- enforcing policy before any peer execution occurs

This is not just raw file transfer. It is policy-aware remote operation dispatch.

## Principal Model

Spacedrive needs a new principal type for agent access.

Suggested model:

```text
AgentPrincipal
  id
  library_id
  kind                 # spacebot
  paired_device_id
  display_name
  created_at
  updated_at
  status
```

This principal represents the Spacebot instance inside the library.

All remote operations performed by Spacebot should be evaluated against this principal.

## Policy Model

Permissions must be library-scoped and target-aware.

Suggested layers:

### Device Access Policy

Determines which devices the Spacebot principal may access.

Examples:

- may access MacBook and NAS
- may not access iPhone

### Location and Subtree Policy

Determines which locations or paths are accessible on those devices.

Examples:

- allow `~/Projects`
- deny `~/Documents/Finance`
- allow NAS media archive as read-only

### Operation Policy

Determines what Spacebot may do there.

Examples:

- `list`
- `read`
- `search`
- `write`
- `move`
- `delete`
- `shell`
- `computer_use`

### Confirmation Policy

Determines which actions require live user confirmation.

Examples:

- destructive actions require confirmation
- shell allowed only in trusted workspace roots
- computer use allowed only on approved desktop devices

## Effective Permission Resolution

For every request, Spacedrive should resolve permissions using:

- `agent_principal`
- `library_id`
- `target_device_id`
- `target_location_or_path`
- `operation_kind`

That resolution should happen on the paired Spacebot node before forwarding, and again on the target device if needed for defense in depth.

## Request Flow

### Example: remote shell command

```text
1. User asks Spacebot to work on a repo on the MacBook
2. Spacebot spawns a worker with execution_target = MacBook
3. Worker calls shell tool
4. Proxy tool sends request to paired Spacedrive node
5. Paired node resolves effective policy for principal + MacBook + path + shell
6. If allowed, paired node forwards typed execution request to MacBook Spacedrive
7. MacBook Spacedrive executes locally inside its OS context
8. Result returns to paired node
9. Result returns to Spacebot worker
10. Worker continues and reports status
```

### Example: remote file access

```text
1. Worker targets NAS device
2. Worker calls file read/list/edit tool
3. Spacedrive permission system checks subtree and capability rules
4. NAS device executes file operation locally
5. Response returns with audit metadata
```

## Capability Surface

Spacedrive should model remote execution as typed capabilities, not ad hoc commands.

Initial capability families:

- filesystem query
- filesystem mutation
- shell execution
- search and indexing queries
- future computer use
- future application integration and automation

This keeps the permission model explicit and inspectable.

## Transport Model

The transport should be a new Spacedrive peer execution protocol.

It should live alongside existing pairing, library sync, file transfer, and remote job activity protocols.

Recommended shape:

- typed operation envelope
- library-scoped principal identity
- target device identity
- capability metadata
- request ID and audit metadata
- result payload and status

The target device should execute through the same internal dispatch and action/query system where practical, not through a completely separate execution stack.

## Audit Model

Every Spacebot-routed operation should be auditable.

Suggested audit fields:

```text
request_id
library_id
agent_principal_id
origin_device_id        # paired Spacebot node
target_device_id
operation_kind
target_path
policy_decision
requires_confirmation
timestamp
result_status
```

This is critical for trust, debugging, and future approval workflows.

## Device Graph Context for Spacebot

Spacebot needs a compact view of the library graph.

It should know:

- what devices exist
- whether they are online
- what capabilities they expose
- which roots are accessible
- which roots are writable or restricted
- high-level policy summaries

This context should be concise for channels and richer for workers.

Do not dump the full graph blindly into every prompt.

## Why This Replaces Spacebot Desktop

If Spacedrive is the permission and execution layer, then Spacedrive is also the right user interface layer.

That means:

- chat happens in Spacedrive
- voice and floating panels happen in Spacedrive
- permission granting happens in Spacedrive
- device selection happens in Spacedrive
- file and context views happen in Spacedrive

Spacebot Desktop becomes redundant once this model exists.

## Local vs Hosted Symmetry

This design intentionally keeps local and hosted deployments symmetric.

### Local

- Spacedrive launches Spacebot locally
- both share one local paired device

### Hosted

- hosted environment runs one Spacebot and one Spacedrive node together
- that node is visible as a device in the user's library
- user pairs the rest of their devices into the same library

Same architecture, different packaging.

## What Already Exists

Spacedrive already has:

- device identity and pairing
- library-scoped device membership
- peer discovery
- cross-device addressing
- file transfer
- typed action/query dispatch
- library-scoped jobs and remote job visibility

Spacebot already has:

- worker lifecycle and isolated tool servers
- local shell and file tools
- alternate worker backend precedent
- strong API and runtime model

The integration does not require rewriting either product from scratch. It requires adding the right boundary.

## What Is Missing

### In Spacedrive

- agent principal identity
- per-device policy model
- per-subtree capability policy
- effective permission resolution
- remote operation forwarding
- audit-first remote execution records

### In Spacebot

- worker execution target abstraction
- Spacedrive-backed proxy tools
- device/capability awareness in worker scheduling
- UI that surfaces device targeting and approvals through Spacedrive

## Recommended Phases

### Phase 1: Principal and Policy Model

- add Spacebot principal type to Spacedrive
- add device, subtree, and capability policies
- add effective-permission resolution

### Phase 2: Remote File Queries

- allow Spacebot-routed list/read/search against target devices through Spacedrive
- wire policy enforcement and audit logging

### Phase 3: Remote Shell Execution

- add shell capability and remote execution protocol
- bind worker shell tool to Spacedrive proxy

### Phase 4: Spacedrive UI as the Agent Surface

- embed chat, voice, target-device status, and permission controls in Spacedrive
- remove dependency on Spacebot Desktop

### Phase 5: Computer Use

- add computer-use capability for approved devices
- route through same principal and policy model

### Phase 6: Hosted Productization

- ship hosted Spacebot with embedded Spacedrive node
- build onboarding around library pairing

## Open Questions

1. Should the target device perform a second policy check locally, or should the paired Spacebot node be the only enforcement point?
2. How should offline devices be represented in worker scheduling and retries?
3. Which operations should always require live approval even when globally allowed?
4. Should execution target be chosen explicitly by the model, by policy, or by a scheduler that picks the best allowed device?
5. How should mobile devices participate, read-only, limited actions, or full capability subsets?

## Recommendation

Adopt the paired-node architecture.

The correct long-term model is:

- one Spacebot instance
- one paired Spacedrive node for that Spacebot
- many user Spacedrive devices in the same library
- Spacedrive as the permission and execution layer
- Spacebot as the agent runtime and scheduler

This gives us a clean answer to a core product question:

- how does one agent operate naturally across a user's whole device fleet without centralizing trust?

By operating through Spacedrive.
