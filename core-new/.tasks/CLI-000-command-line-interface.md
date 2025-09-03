---
id: CLI-000
title: "Epic: Command-Line Interface"
status: To Do
assignee: unassigned
priority: High
tags: [epic, cli]
whitepaper: "N/A"
---

## Description

This epic covers the development of the Spacedrive command-line interface (CLI), providing users with a way to interact with the system from the terminal.

## Implementation Notes

- The CLI should be built using the `clap` crate for parsing arguments and subcommands.
- It should have a clear and consistent command structure.
- Commands should be implemented for core functionalities such as:
  - `status`: Displaying the status of the Spacedrive daemon.
  - `index`: Triggering indexing of locations.
  - `add-location`: Adding new locations to be indexed.
  - `list-locations`: Listing all indexed locations.

## Acceptance Criteria

- The CLI can be built and run successfully.
- The CLI provides a set of commands for interacting with the system.
- The CLI is well-documented and easy to use.
