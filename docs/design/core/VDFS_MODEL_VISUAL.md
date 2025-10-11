<!--CREATED: 2025-06-18-->
# VDFS Domain Model - Visual Overview

## Core Relationships

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Virtual Distributed File System                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Device A (MacBook)                    Device B (iPhone)                │
│  ┌─────────────────┐                  ┌─────────────────┐             │
│  │ id: aaaa-bbbb   │                  │ id: 1111-2222   │             │
│  │ name: MacBook   │◄─────P2P────────►│ name: iPhone    │             │
│  │ os: macOS       │                  │ os: iOS         │             │
│  └─────────────────┘                  └─────────────────┘             │
│           │                                     │                       │
│           ▼                                     ▼                       │
│  ┌─────────────────┐                  ┌─────────────────┐             │
│  │   Location      │                  │   Location      │             │
│  │ "My Documents"  │                  │ "Camera Roll"   │             │
│  │ /Users/me/Docs  │                  │ /DCIM/          │             │
│  └─────────────────┘                  └─────────────────┘             │
│           │                                     │                       │
│           ▼                                     ▼                       │
│  ┌─────────────────┐                  ┌─────────────────┐             │
│  │     Entry       │                  │     Entry       │             │
│  │ "photo.jpg"     │                  │ "IMG_1234.jpg"  │             │
│  │ device: aaaa    │                  │ device: 1111    │             │
│  │ path: /Docs/... │                  │ path: /DCIM/... │             │
│  └────────┬────────┘                  └────────┬────────┘             │
│           │                                     │                       │
│           ▼                                     ▼                       │
│  ┌─────────────────┐                  ┌─────────────────┐             │
│  │  UserMetadata   │                  │  UserMetadata   │             │
│  │ tags: [Vacation]│                  │ tags: []        │             │
│  │ favorite: true  │                  │ favorite: false │             │
│  └─────────────────┘                  └─────────────────┘             │
│           │                                     │                       │
│           └──────────────┬─────────────────────┘                       │
│                          ▼                                              │
│                 ┌─────────────────┐                                    │
│                 │ ContentIdentity  │                                    │
│                 │ cas_id: v2:a1b2  │ (Same content, different devices) │
│                 │ kind: Image      │                                    │
│                 │ entry_count: 2   │                                    │
│                 └─────────────────┘                                    │
└─────────────────────────────────────────────────────────────────────────┘
```

## Key Concepts Illustrated

### 1. SdPath in Action
```
SdPath { 
    device_id: "aaaa-bbbb",
    path: "/Users/me/Documents/photo.jpg"
}
// This uniquely identifies a file across all devices!
```

### 2. Entry Always Has UserMetadata
```
Entry ──────────► UserMetadata
(always)          (can tag immediately!)
   │
   └─────────────► ContentIdentity
   (optional)     (for deduplication)
```

### 3. Progressive Enhancement Flow
```
Step 1: Discover File
├─ Create Entry
└─ Create UserMetadata (empty)
    └─ User can tag immediately! ✓

Step 2: Index Content (optional, async)
├─ Generate CAS ID
├─ Create/Link ContentIdentity
└─ Enable deduplication ✓

Step 3: Deep Index (optional, background)
├─ Extract text for search
├─ Generate thumbnails
└─ Extract media metadata ✓
```

### 4. Cross-Device Operations
```
copy_files(
    source: SdPath { device: "macbook", path: "/photo.jpg" },
    dest:   SdPath { device: "iphone", path: "/Photos/" }
)
// The system handles all P2P complexity transparently!
```

## Benefits Visualized

### Old Model Problems
```
File → Object (requires CAS ID) → Tags
         Can't tag without indexing!
```

### New Model Solution
```
Entry → UserMetadata → Tags
  │        ✓ Immediate tagging!
  └────► ContentIdentity (optional)
           ✓ Deduplication when needed
```

### Content Change Handling
```
Before: photo.jpg → Edit → New CAS ID → Lost tags! ❌

After:  Entry → UserMetadata (unchanged) ✓
          │         Tags preserved!
          └────► New ContentIdentity
```

## Real-World Scenarios

### Scenario 1: Tag Before Index
```
1. User drops 1000 photos into Spacedrive
2. Immediately tags them "Vacation 2024" (instant!)
3. Content indexing happens in background
4. Deduplication available when ready
```

### Scenario 2: Cross-Device Sync
```
1. Tag photos on MacBook
2. Photos sync to iPhone with tags intact
3. Edit photo on iPhone
4. Tags remain, content identity updates
5. Both devices see the same tags
```

### Scenario 3: Removable Media
```
1. Insert USB drive
2. Browse and tag files (no indexing needed)
3. Remove USB drive
4. Tags remembered for when drive returns
5. Virtual entries maintain metadata
```

This architecture makes Spacedrive's Virtual Distributed File System a reality!