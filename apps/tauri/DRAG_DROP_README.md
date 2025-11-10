# Native Drag & Drop System

A production-ready native drag-and-drop implementation for Spacedrive using AppKit on macOS.

## Features

- **Native OS Integration**: Real `NSDraggingSession` - files can be dropped into Finder, other apps
- **Custom Overlay**: User-controlled React component follows cursor during drag
- **Multi-Window Support**: Drag state synchronized across all Spacedrive windows via Tauri events
- **Live Updates**: Overlay can react to drag events in real-time
- **Type-Safe**: Full TypeScript definitions matching Rust types
- **File Promises**: Support for virtual files generated on-drop

## Quick Start

### 1. Start a Drag Operation

```typescript
import { useDragOperation } from './hooks/useDragOperation';

function MyComponent() {
  const { startDrag, isDragging } = useDragOperation({
    onDragStart: (sessionId) => console.log('Started:', sessionId),
    onDragEnd: (result) => console.log('Result:', result),
  });

  const handleDrag = async () => {
    await startDrag({
      items: [
        {
          id: 'file-1',
          kind: { type: 'File', path: '/path/to/file.pdf' }
        }
      ],
      allowedOperations: ['copy', 'move'],
    });
  };

  return <button onClick={handleDrag}>Drag File</button>;
}
```

### 2. Create a Drop Zone

```typescript
import { useDropZone } from './hooks/useDropZone';

function DropTarget() {
  const { isHovered, dropZoneProps } = useDropZone({
    onDrop: (items) => console.log('Dropped:', items),
  });

  return (
    <div
      {...dropZoneProps}
      className={isHovered ? 'border-blue-500' : 'border-gray-300'}
    >
      Drop files here
    </div>
  );
}
```

### 3. Customize the Drag Overlay

The overlay component at `/drag-overlay` renders during drag operations:

```typescript
// apps/tauri/src/routes/DragOverlay.tsx
export function DragOverlay() {
  const [session, setSession] = useState<DragSession | null>(null);

  useEffect(() => {
    getDragSession().then(setSession);
  }, []);

  return (
    <div className="custom-drag-preview">
      {session?.config.items.length} files
    </div>
  );
}
```

## Architecture

```
React App                     Rust/Tauri                  macOS (Swift)
┌─────────────┐              ┌──────────────┐            ┌───────────────┐
│             │              │              │            │               │
│ startDrag() ├─invoke────► │ begin_drag   ├─FFI─────► │ beginNative   │
│             │              │              │            │ Drag          │
│             │              │              │            │               │
│             │              │ DragCoord    │            │ NSDragging    │
│             │              │ inator       │            │ Source        │
│             │   ◄──────────┤              ◄───NSNotif──┤               │
│ onDragMoved │   emit event │ emit to all  │            │ draggingMoved │
│             │              │ windows      │            │               │
└─────────────┘              └──────────────┘            └───────────────┘
```

## API Reference

### `useDragOperation(options)`

Hook for initiating drag operations.

**Options:**
- `onDragStart?: (sessionId: string) => void`
- `onDragMove?: (x: number, y: number) => void`
- `onDragEnd?: (result: DragResult) => void`

**Returns:**
- `isDragging: boolean`
- `currentSession: DragSession | null`
- `cursorPosition: { x: number; y: number } | null`
- `startDrag: (config) => Promise<string>`
- `cancelDrag: (sessionId) => Promise<void>`

### `useDropZone(options)`

Hook for creating drop targets.

**Options:**
- `onDrop?: (items: DragItem[]) => void`
- `onDragEnter?: () => void`
- `onDragLeave?: () => void`

**Returns:**
- `isHovered: boolean`
- `dragItems: DragItem[]`
- `dropZoneProps: object` - spread onto your drop zone element

### Types

```typescript
type DragItemKind =
  | { type: 'File'; path: string }
  | { type: 'FilePromise'; name: string; mimeType: string }
  | { type: 'Text'; content: string };

interface DragItem {
  kind: DragItemKind;
  id: string;
}

interface DragConfig {
  items: DragItem[];
  overlayUrl: string;
  overlaySize: [number, number];
  allowedOperations: DragOperation[];
}
```

## Demo

Run the demo window:

```bash
# From apps/tauri
bun run dev
```

Then open the drag demo by changing the main window label to `drag-demo` or by programmatically showing it:

```typescript
import { invoke } from '@tauri-apps/api/core';

await invoke('show_window', {
  window: { type: 'Main' } // Opens the demo
});
```

## Implementation Details

### Rust Layer (`src-tauri/src/drag/`)

- **`mod.rs`**: `DragCoordinator` - global state manager
- **`session.rs`**: Session tracking with UUIDs
- **`events.rs`**: Type-safe event definitions
- **`commands.rs`**: Tauri command handlers

### Swift Layer (`crates/macos/src-swift/drag.swift`)

- **`NativeDragSource`**: Implements `NSDraggingSource` protocol
- **File Promises**: Implements `NSFilePromiseProviderDelegate`
- **Notification Bridge**: Sends events back to Rust via `NotificationCenter`

### TypeScript Layer (`src/`)

- **`lib/drag.ts`**: Low-level API wrappers
- **`hooks/useDragOperation.ts`**: Drag initiation hook
- **`hooks/useDropZone.ts`**: Drop target hook
- **`routes/DragOverlay.tsx`**: Cursor-following overlay component

## Known Limitations

- **macOS only** - Windows/Linux support not yet implemented
- **Overlay mouse events**: Currently the overlay ignores all mouse events (by Tauri config, not objc2 calls)
- **File promise callbacks**: Requires implementing file generation logic via NSNotification

## Future Enhancements

1. **Windows Support**: Implement OLE drag-drop
2. **Linux Support**: X11/Wayland integration
3. **Bidirectional Drop**: Handle drops FROM external apps INTO Spacedrive
4. **Drag Modifiers**: Support Cmd/Ctrl for copy vs. move
5. **Multi-Monitor**: Better positioning for multi-display setups

## Troubleshooting

**Drag doesn't start:**
- Check console for Rust errors
- Ensure `begin_drag` command is registered in `main.rs`
- Verify Swift build succeeded

**Overlay doesn't appear:**
- Check `DragOverlay` window is created
- Verify `/drag-overlay` route exists
- Check for TypeScript errors in overlay component

**Drop doesn't work:**
- Ensure `useDropZone` hook is mounted before drag starts
- Check window labels match between drag source and drop target
- Verify event listeners are properly cleaned up

## Contributing

This is an experimental feature. Contributions welcome, especially for:
- Windows/Linux platform support
- Better error handling
- Performance optimizations
- Additional drag item types

## License

Same as Spacedrive project.
