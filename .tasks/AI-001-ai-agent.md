---
id: AI-001
title: Develop AI Agent for Proactive Assistance
status: To Do
assignee: james
parent: AI-000
priority: High
tags: [ai, agent, core]
whitepaper: Section 4.6
---

## Description

Implement the core AI agent that observes user behavior and proactively suggests helpful actions. The agent will use the VDFS index as its "world model" and the Action System as its method for interacting with the user's data.

## Implementation Steps

1.  Create a service that analyzes the `audit_log` table to identify user patterns (e.g., frequently moving certain file types to specific folders).
2.  Develop a mechanism for the agent to generate a structured `Action` (e.g., a `FileCopyAction`) based on its analysis.
3.  Implement a suggestion system where the agent's proposed action is presented to the user as a pre-visualized preview for one-click approval.
4.  Integrate with local models via `Ollama` for privacy-first analysis.

## Acceptance Criteria

- [ ] The agent can detect when a user repeatedly performs the same organizational task.
- [ ] The agent can propose a valid, pre-visualized `Action` to automate that task.
- [ ] The user can approve or deny the suggestion.
