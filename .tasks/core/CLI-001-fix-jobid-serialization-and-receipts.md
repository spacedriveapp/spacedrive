---
id: CLI-001
title: "Fix JobId transparent serialization and CLI receipt parsing"
status: "Done"
assignee: "jamiepine"
priority: "High"
tags: ["core", "cli", "jobs"]
---

## Description

CLI commands that interact with the daemon's job system (such as indexing a library or copying files) were failing with JSON deserialization errors. The CLI submitted the job, but crashed when parsing the response. 

The underlying cause was a mismatch between the serialization format of the `JobId` struct and the daemon's response shape. The daemon returns a `JobReceipt` (which includes the job ID and name), while the CLI was trying to parse a bare `JobId`. Furthermore, `JobId` was serializing as a newtype struct (e.g., `{"JobId": "..."}`) instead of a raw UUID string.

## The Why

In a CQRS architecture, the CLI and daemon must strictly agree on the RPC input and output shapes. 
1. `JobId` is a newtype wrapper around `Uuid`. By default, Serde serializes this as a tuple struct. By marking it `#[serde(transparent)]`, we force it to serialize as a standard string UUID, ensuring compatibility over the wire.
2. The daemon's `execute_action!` macro returns the full `JobReceipt`, not just the `JobId`. The CLI parsing logic must be updated to expect this receipt to avoid dropping the connection.

## The How (Implementation Steps)

1.  **Transparent Serialization**:
    We applied the `#[serde(transparent)]` macro to `core::infra::job::types::JobId`.
    ```rust
    /// Unique identifier for a job
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
    #[serde(transparent)]
    pub struct JobId(pub Uuid);
    ```
2.  **CLI Receipt Parsing**:
    We updated `apps/cli/src/domains/index/mod.rs` and `apps/cli/src/domains/file/mod.rs` to expect a `JobReceipt` instead of `JobId`.

### Example Diff

```diff
- let out: JobId = execute_action!(ctx, input);
- print_output!(ctx, out, |_| {
-     println!("Browse request submitted");
- });
+ let out: JobReceipt = execute_action!(ctx, input);
+ print_output!(ctx, &out, |r: &JobReceipt| {
+     println!("Browse job submitted: {} (job: {})", r.id, r.job_name);
+ });
```

## Acceptance Criteria
- Submitting an indexing or file copy job via CLI parses the receipt successfully.
- The `JobId` standardizes to a transparent UUID format across the JSON-RPC boundary.
- The CLI outputs both the Job ID and Job Name upon successful submission.

## Review Refinements
- **Import Organization:** Reordered imports in `apps/cli/src/domains/file/mod.rs` and `apps/cli/src/domains/index/mod.rs` to ensure external `sd_core` dependencies are grouped properly with other external crates, separated from local `crate::` imports by a blank line.
