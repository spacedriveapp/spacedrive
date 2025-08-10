# Design Document: The Spacedrive Activity Center

## 1. Overview

This document outlines the design for the **Spacedrive Activity Center**, a next-generation user interface for monitoring and managing all background tasks and system operations. Inspired by the file transfer dialogs in native operating systems, the Activity Center reimagines this concept as a beautiful, interactive, and radically transparent "mission control" for the entire VDFS.

It provides a single, unified view of file operations, compute jobs (indexing, thumbnailing), real-time sync status, and actions taken by AI agents.

## 2. Design Principles

- **Radical Transparency:** Expose all system operations—whether initiated by the user, the system, or an AI agent—in a beautiful and understandable way.
- **Aesthetic Excellence:** Create a UI that is not just functional but "gorgeous" and desirable to have open on a desktop.
- **Interactive & Dynamic:** The UI is not a static list. It is a live window into the system with animated graphs, real-time metrics, and fluid interactions.
- **Modular & Composable:** Core UI components are designed to be assembled in different ways, allowing for a native multi-window experience on desktop and a sophisticated single-page experience on the web.
- **Unified View:** Consolidate file operations, sync status, compute jobs, and agent activity into a single, coherent interface.

## 3. Architectural Components of the UI

The Activity Center is composed of three main components that work together to create a rich, informative experience.

### 3.1. The Live Resource Component

This is the iconic element at the top of the view, providing an at-a-glance summary of system resource usage.

- **Structure:** A set of sleek, minimalist bars, each representing a key resource:
  - **Network:** Combined upload/download activity.
  - **Disk:** Read/write activity across all tracked locations.
  - **CPU/Compute:** Usage for intensive tasks like indexing or transcoding.
  - **Sync:** The rate of synchronization operations between devices.
- **Interaction:**
  - At rest, the bars show a subtle, real-time percentage of usage.
  - On click or hover, a bar fluidly **expands horizontally to fill the width of the view.** This reveals a beautiful, animated historical graph of that resource's usage, with the current transfer/processing speed as the primary, bold metric.

### 3.2. The Unified Event Stream

A chronological, and infinitely scrollable timeline of every significant event occurring across the VDFS. This is the user-facing view of the Action System.

- **Content:** Each item in the stream is an "event card" representing a single action, clearly distinguished by icons and context:
  - **File Operations:** `[Copy Icon] Copied 3,402 items from 'iPhone' to 'NAS'.`
  - **Compute Jobs:** `[Index Icon] Indexing '~/Documents' finished.`
  - **Agent Actions:** `[Agent Avatar] AI Assistant is organizing 'Project Phoenix'...`
  - **Sharing:** `[Spacedrop Icon] Sent 'presentation.mov' to 'Colleague's MacBook'.`
- **Interaction:**
  - Events appear in real-time as they are dispatched by the backend.
  - Clicking on any event card smoothly navigates the user to the **Detailed Job View** for that specific action.

### 3.3. The Detailed Job View

This is the drill-down view for a single job or action.

- **Structure:** It's a focused view that combines the other two components.
  - At the top, it features the **Live Resource Dashboard**, but now **scoped to show only the resources being used by that specific job**.
  - Below, it shows detailed progress (e.g., a file list, percentage complete, ETA), logs, and controls (Pause, Resume, Cancel).

## 4. The Composable UI Philosophy

This design embraces a modular, "post-tab" interface that can be adapted for different platforms.

### 4.1. Multi-Window Experience

- **On Desktop (macOS, Windows):** The Activity Center can be a primary window, a menu bar applet, or individual job views can be "popped out" into their own separate, lightweight native windows. This allows a user to arrange their workspace for a true "mission control" feel.
- **On the Web:** The same components can be assembled within a "virtual desktop" environment inside the browser tab. The floating windows and panels would be simulated, providing a consistent experience without relying on native OS windowing.

### 4.2. Dynamic Layout Management: Free-form vs. Automatic

To give users both ultimate control and intelligent organization, the virtual desktop will support two distinct layout modes the user can toggle between at any time.

1.  **Free-form Mode (Your Workbench):**

    - This is the user's persistent, custom layout. They can drag, resize, and arrange all floating "applets" (file explorers, the Activity Center, etc.) in any way that suits their workflow.
    - The size and position of every window are saved, so the user's personalized workspace is always exactly as they left it.

2.  **Automatic Modes (Task-Oriented Layouts):**
    - These are predefined, clean layouts optimized for specific tasks, selectable from a menu.
    - Examples: "Focus Mode" (one file browser maximized), "Organization Mode" (two file browsers side-by-side), "Activity Mode" (Activity Center maximized).

**The Animated Transition:**

The transition between these modes is seamless. When a user switches from an "Automatic" layout back to "Free-form," the system remembers the user's last custom positions. Each panel will **fluidly animate from its organized spot back to its unique, user-defined "home,"** creating a delightful and spatially intuitive experience.

## 5. Backend Integration

This UI is powered directly by the existing backend architecture:

- **The Unified Event Stream** is a direct visual representation of events received from the core `EventBus`.
- **The Detailed Job View** gets its real-time progress data by subscribing to the `JobContext` updates for a specific job.
- The resource usage data for the **Live Resource Dashboard** will be provided by new metrics exposed by the core services (e.g., Networking, Job System).

## 6. Implementation Plan

1.  **Phase 1: Foundation & Data Hooks:** Build the basic Activity Center window. Implement the backend logic to expose resource metrics and connect the UI to the `EventBus` and `JobManager` to receive live data.
2.  **Phase 2: The Unified Event Stream:** Build the "event card" UI and the chronological, scrollable list.
3.  **Phase 3: The Detailed Job View & Resource Dashboard:** Build the drill-down view for individual jobs. Implement the expanding resource bars and the animated historical graphs.
4.  **Phase 4: Composable Windowing & Layouts:** Implement the virtual desktop shell, the pop-out window functionality (for desktop), and the dynamic layout management system.
