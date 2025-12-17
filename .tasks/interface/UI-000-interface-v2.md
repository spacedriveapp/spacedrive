---
id: UI-000
title: "Epic: Interface V2 Architecture"
status: In Progress
assignee: jamiepine
priority: High
tags: [epic, interface, react]
last_updated: 2025-12-02
---

## Description

Complete rewrite of the Spacedrive interface using React 19, TypeScript, and a clean component architecture. The interface is platform-agnostic and works across Tauri (desktop), web, and mobile.

## Key Principles

- Platform agnostic architecture
- Type-safe client with auto-generated types from Rust
- Semantic color system with Tailwind
- Clean separation: @sd/interface (features) + @sd/ui (primitives) + @sd/ts-client (state)
- Accessible, performant, production-ready

## Implementation Notes

- Built with React 19, TanStack Query, Framer Motion
- Uses native macOS traffic lights (no CSS fakes)
- V2 design is more rounded than V1 (rounded-lg vs rounded-md)
- All colors use semantic Tailwind classes, never `var()` directly

## Acceptance Criteria

- [x] Type-safe client with auto-generated types
- [x] Native macOS traffic lights working
- [x] V2 color system as CSS variables
- [x] TanStack Query integration
- [ ] Complete Explorer with file operations
- [ ] Settings pages functional
- [ ] Multi-window support
- [ ] Mobile app integration
