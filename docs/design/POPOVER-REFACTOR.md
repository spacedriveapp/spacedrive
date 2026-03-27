# Popover Refactor: Wrapped → Radix Composable

## The Problem

9 files use the old wrapped Popover API:
```tsx
<Popover popover={popover} trigger={<button>...</button>} side="top" className="...">
  {children}
</Popover>
```

`Popover` is now a Radix composable object from `@spaceui/primitives`, not a function component. Every usage must be converted.

## The Pattern

**Before:**
```tsx
import { Popover, usePopover } from "@spaceui/primitives";

const popover = usePopover();

<Popover
  popover={popover}
  trigger={<button>Open</button>}
  side="top"
  align="start"
  sideOffset={8}
  className="w-[300px]"
>
  {children}
</Popover>
```

**After:**
```tsx
import { Popover, usePopover } from "@spaceui/primitives";

const popover = usePopover();

<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
  <Popover.Trigger asChild>
    <button>Open</button>
  </Popover.Trigger>
  <Popover.Content side="top" align="start" sideOffset={8} className="w-[300px]">
    {children}
  </Popover.Content>
</Popover.Root>
```

## Rules

1. `popover={popover}` → `open={popover.open} onOpenChange={popover.setOpen}` on `Popover.Root`
2. `trigger={<Component />}` → wrap in `<Popover.Trigger asChild><Component /></Popover.Trigger>`
3. `side`, `align`, `sideOffset`, `alignOffset`, `className` move to `<Popover.Content>`
4. Children of the old `<Popover>` become children of `<Popover.Content>`
5. Keep `usePopover()` — it still works

## Files to Refactor

1. `packages/interface/src/Spacebot/ChatComposer.tsx`
2. `packages/interface/src/Spacebot/SpacebotLayout.tsx`
3. `packages/interface/src/Spacebot/routes/ChatRoute.tsx`
4. `packages/interface/src/routes/explorer/components/PathBar.tsx`
5. `packages/interface/src/routes/overview/OverviewTopBar.tsx`
6. `packages/interface/src/components/SyncMonitor/SyncMonitorPopover.tsx`
7. `packages/interface/src/components/JobManager/JobManagerPopover.tsx`
8. `packages/interface/src/components/Tags/TagSelector.tsx`
9. `packages/interface/src/windows/VoiceOverlay.tsx`

Search for `<Popover` followed by `popover=` in each file and apply the pattern above.
