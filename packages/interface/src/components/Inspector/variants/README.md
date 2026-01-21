# Inspector Variants

The Inspector has been refactored to support multiple resource types using composition instead of conditionals.

## Architecture

```
Inspector.tsx                  # Generic container
├── FileInspector.tsx         # File variant
├── LocationInspector.tsx     # Location variant
├── DeviceInspector.tsx       # Device variant (future)
├── VolumeInspector.tsx       # Volume variant (future)
└── shared/                   # Shared tab components
    ├── ActivityTab.tsx       # (future)
    └── ChatTab.tsx           # (future)
```

## Usage

### Basic Example

```tsx
import { Inspector, type InspectorVariant } from '@sd/interface';

function MyComponent() {
  const [variant, setVariant] = useState<InspectorVariant>(null);

  // When a file is selected
  function handleFileSelect(file: File) {
    setVariant({ type: 'file', file });
  }

  // When a location is viewed (no file selected)
  function handleLocationView(location: LocationInfo) {
    setVariant({ type: 'location', location });
  }

  // Clear selection
  function handleClear() {
    setVariant({ type: 'empty' });
  }

  return (
    <div className="flex">
      {/* Your content */}
      <div className="flex-1">...</div>

      {/* Inspector */}
      <div className="w-[280px]">
        <Inspector
          variant={variant}
          onPopOut={handlePopOut}
          showPopOutButton={true}
        />
      </div>
    </div>
  );
}
```

## Variant Types

```typescript
type InspectorVariant =
  | { type: 'file'; file: File }
  | { type: 'location'; location: LocationInfo }
  | { type: 'device'; device: DeviceInfo }      // Future
  | { type: 'volume'; volume: VolumeInfo }      // Future
  | { type: 'empty' }
  | null;
```

## File Inspector Tabs

- **Overview**: File details, metadata, AI processing
- **Sidecars**: Generated thumbnails, derivatives
- **Instances**: All copies across devices
- **Chat**: Collaboration (demo)
- **Activity**: File history
- **Details**: Technical metadata

## Location Inspector Tabs

- **Overview**: Location stats, quick actions
- **Indexing**: Index mode, ignore rules
- **Jobs**: Configure automatic processing jobs
- **Activity**: Scan history, job logs
- **Devices**: Devices with access to location
- **More**: Advanced settings, danger zone

## Adding New Variants

1. Create `[Resource]Inspector.tsx` in `inspectors/`
2. Define tabs and tab components
3. Add variant to `InspectorVariant` union type
4. Add case to `Inspector.tsx` container
5. Export from `inspectors/index.ts`

### Example: DeviceInspector

```tsx
// inspectors/DeviceInspector.tsx
import { useState } from 'react';
import { Info, HardDrive } from '@phosphor-icons/react';
import type { DeviceInfo } from '@sd/ts-client';

export function DeviceInspector({ device }: { device: DeviceInfo }) {
  const [activeTab, setActiveTab] = useState('overview');

  const tabs = [
    { id: 'overview', label: 'Overview', icon: Info },
    { id: 'storage', label: 'Storage', icon: HardDrive },
    // ... more tabs
  ];

  return (
    <>
      <Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />
      <div className="flex-1 overflow-hidden flex flex-col mt-2.5">
        <TabContent id="overview" activeTab={activeTab}>
          <OverviewTab device={device} />
        </TabContent>
        {/* ... more tabs */}
      </div>
    </>
  );
}
```

Then update `Inspector.tsx`:

```tsx
// Inspector.tsx
import { DeviceInspector } from './inspectors/DeviceInspector';

export type InspectorVariant =
  | { type: 'file'; file: File }
  | { type: 'location'; location: LocationInfo }
  | { type: 'device'; device: DeviceInfo }  // Add this
  | { type: 'empty' }
  | null;

// In render:
{variant.type === 'device' ? (
  <DeviceInspector device={variant.device} />
) : ...}
```

## Shared Components

All inspectors use shared primitives from `components/Inspector/`:

- `<Section>` - Container with title and icon
- `<InfoRow>` - Label/value pair
- `<Tabs>` - Tab navigation
- `<TabContent>` - Tab content wrapper
- `<Divider>` - Horizontal divider
- `<Tag>` - Colored tag badge

## Benefits

**Clean separation** - Each variant is self-contained
**Type-safe** - Union types ensure correct data
**Extensible** - Add variants without touching existing code
**Reusable** - Shared primitives reduce duplication
**Maintainable** - No conditional rendering mess