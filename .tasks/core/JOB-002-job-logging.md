---
id: JOB-002
title: Job-Specific File Logging
status: Done
assignee: james
parent: JOB-000
priority: Medium
tags: [core, jobs, logging]
whitepaper: Section 6
---

## Description

A dedicated logging system for jobs will be implemented. When enabled, each job writes its detailed operational logs, including progress and debug messages, to a unique log file.

## Implementation Notes

- The logging logic will be implemented in `src/infrastructure/jobs/logger.rs`.
- Configuration is managed via `JobLoggingConfig` in `src/config/app_config.rs`.
- The `JobExecutor` creates a `FileJobLogger` instance for each job it runs, passing it down through the `JobContext`.
- Log files are stored in the `job_logs` directory within the library path.

## Acceptance Criteria

- [x] When enabled in the config, running a job creates a corresponding `.log` file.
- [x] Progress, info, and error messages from the job context are written to the file.
- [x] The logger respects the `include_debug` configuration flag.
