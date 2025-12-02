---
id: UI-001
title: Core Component Primitives
status: In Progress
assignee: james
parent: UI-000
priority: High
tags: [ui, components, design-system]
whitepaper: N/A
last_updated: 2025-12-02
---

## Description

Build a complete set of reusable UI primitives in @sd/ui that follow the V2 design system. These are platform-agnostic, accessible components used throughout the interface.

## Components

**Completed:**
- DropdownMenu (expanding, not overlay)
- Button variants

**In Progress:**
- Dialog/Modal
- Tooltip
- Input variants
- Checkbox/Radio
- Slider

**Planned:**
- Tabs
- Accordion
- Select
- Combobox
- Progress
- Badge
- Avatar

## Implementation Notes

- Built on Radix UI for accessibility
- Minimal styling in primitives (styled via className prop)
- Semantic color system support
- Framer Motion for animations
- Platform-agnostic (works on all platforms)

## Acceptance Criteria

- [x] DropdownMenu with expanding animation
- [x] Button with variants (primary, secondary, danger)
- [ ] Dialog with backdrop blur
- [ ] Tooltip with proper positioning
- [ ] Input with validation states
- [ ] Form components (checkbox, radio, slider)
- [ ] All components keyboard accessible
- [ ] All components follow V2 rounded style
- [ ] Documented in Storybook or docs
