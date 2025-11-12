# Drag & Drop Fixes Applied

## Problem
Drag-and-drop worked perfectly on the first attempt but failed on subsequent drags with no apparent error.

## Root Cause
The `DragCoordinator` in Rust maintained a state lock that was never properly cleared after the first drag completed. When attempting a second drag, the guard clause `if state.is_some()` would immediately reject it with "A drag operation is already in progress".

## Fixes Applied

### 1. Rust State Management (`apps/tauri/src-tauri/src/drag/mod.rs`)

**Added timeout-based stale state cleanup:**
- Changed state from `Option<DragSession>` to `Option<(DragSession, Instant)>` to track when drags start
- Added automatic cleanup of stale sessions older than 30 seconds
- This prevents permanent state lock if `end_drag` is never called

**Added comprehensive logging:**
- Log when drag sessions start with session ID
- Log when drag sessions end with duration and result
- Warn when `end_drag` is called but no active session exists
- Error logging when state is already locked

**Added force cleanup method:**
- New `force_clear_state()` method for emergency state reset
- Emits a cancelled drag event to notify listeners

### 2. Error Path Cleanup (`apps/tauri/src-tauri/src/drag/commands.rs`)

**Enhanced error handling in `begin_drag`:**
- If native drag fails after state is set, now calls `force_clear_state()`
- Properly closes overlay window on failure
- Prevents state from getting stuck when Swift drag initialization fails

**Improved logging:**
- Log all begin_drag calls with window label
- Log Swift native drag results
- Log overlay window cleanup (success and failure cases)

**Added debug command:**
- New `force_clear_drag_state` Tauri command for manual state reset
- Useful for debugging and recovery scenarios

### 3. TypeScript Hook Fixes

**Fixed `useDropZone` hook (`apps/tauri/src/hooks/useDropZone.ts`):**
- Moved callbacks to refs to prevent effect re-runs
- Removed `isHovered` and `dragItems` from dependency array (they were causing the effect to re-run during drags)
- Used functional state updates in `onDragEnded` to access current state without dependencies
- Now event listeners are only set up once per mount, preventing duplicate listeners

**Fixed `useDragOperation` hook (`apps/tauri/src/hooks/useDragOperation.ts`):**
- Moved callbacks to refs
- Removed all callback dependencies from the effect array
- Event listeners now persist for the component lifetime

### 4. Swift Logging (`apps/tauri/crates/macos/src-swift/drag.swift`)

**Added comprehensive logging:**
- Log when `end_native_drag` is called
- Log when drag sessions end with type and operation
- Warn when drag source is not found in active sources
- Log cleanup completion

## Testing the Fixes

Run the app and check console logs:

```bash
cd apps/tauri
bun run dev
```

You should now see logs like:
- `[DRAG] Starting drag session: session_id=...`
- `[DRAG] Drag session ended: session=..., type=dropped/cancelled`
- `Ending drag session: session_id=..., duration=...`

If a drag gets stuck (should be rare now), you can manually clear it:

```typescript
import { invoke } from '@tauri-apps/api/core';
await invoke('force_clear_drag_state');
```

## Critical Fix: Swift-to-Rust Callback Bridge

**Added direct callback from Swift to Rust** (`apps/tauri/crates/macos/src/lib.rs` and `drag.swift`):

The root cause was that Swift's `draggingSession(_:endedAt:)` delegate posted a notification but nothing was listening. JavaScript wasn't reliably calling `endDrag`.

**Solution:**
- Created `rust_drag_ended_callback` FFI function in Rust
- Swift now directly calls this when drag ends
- Rust callback triggers `coordinator.end_drag()` with proper result
- State is guaranteed to be cleared when macOS completes the drag

This ensures the state cleanup happens automatically, regardless of whether JavaScript calls `endDrag` or not.

## Changes Summary

**Modified Files:**
- `apps/tauri/src-tauri/src/drag/mod.rs` - State management with timeout
- `apps/tauri/src-tauri/src/drag/commands.rs` - Error handling and logging
- `apps/tauri/src-tauri/src/main.rs` - Registered callback and new command
- `apps/tauri/src/hooks/useDropZone.ts` - Fixed dependency array
- `apps/tauri/src/hooks/useDragOperation.ts` - Fixed dependency array
- `apps/tauri/crates/macos/src/lib.rs` - Added FFI callback infrastructure
- `apps/tauri/crates/macos/src-swift/drag.swift` - Direct Rust callback on drag end

**Lines of code changed:** ~200 lines across 7 files
