# Spacedrive Event System & Navigation Documentation

This directory contains comprehensive documentation about the resource event system, location creation, and navigation implementation in the Spacedrive interface.

---

## Documentation Files

### 1. **SEARCH_FINDINGS_SUMMARY.md** (9.9 KB) - START HERE
**Best for:** Quick overview of all findings

Answers the 4 key questions:
- Where are resource events received/handled?
- How are location creation events structured?
- How is navigation implemented?
- What event listeners exist?

Includes complete event flow diagrams and key files list.

---

### 2. **ARCHITECTURE_EVENT_NAVIGATION.md** (14 KB) - COMPREHENSIVE REFERENCE
**Best for:** Deep understanding of the entire system

10 detailed sections covering:
1. Event system architecture with flow diagrams
2. Location creation event structures (2 types)
3. Event handling patterns (2 major approaches)
4. Current location creation flow (timeline)
5. Navigation system with React Router details
6. 3 architectural decision approaches
7. All key files and absolute paths
8. Event structure examples (JSON)
9. Code patterns to follow
10. Complete summary

Includes code examples, architectural decisions, and migration guidance.

---

### 3. **IMPLEMENTATION_PATTERNS.md** (11 KB) - CODE EXAMPLES
**Best for:** Copy-paste ready code patterns

6 complete implementation patterns:
1. Listen for event in component
2. Wait for event after mutation
3. Auto-update list from events (recommended)
4. useEvent hook (manual)
5. Event filtering
6. Batch event handling

Also includes:
- Complete working example with AddLocationModal + LocationsSection
- Debugging guide
- Key points to remember
- Files to modify

---

### 4. **CODE_MAP.md** (12 KB) - LINE-BY-LINE REFERENCE
**Best for:** Finding exact code locations

Detailed breakdown of every relevant file with:
- Line numbers for key sections
- Function signatures
- Comments about what each section does
- Current issues/gaps

Key sections:
- ts-client package (5 files)
- interface package (7 files)
- Line numbers for all important code

Includes files-to-modify checklist and testing points.

---

### 5. **EVENT_SYSTEM_SUMMARY.txt** (15 KB) - VISUAL GUIDE
**Best for:** Quick visual reference

ASCII art diagrams and bullet points covering:
1. Resource events received (with diagram)
2. Location creation event structure
3. Navigation implementation
4. Event listeners and handlers
5. Current flow timeline
6. Key files with absolute paths
7. Recommended implementation approaches
8. Important notes
9. Complete summary

Uses visual formatting for quick scanning.

---

## Quick Navigation

### If you want to...

**Understand the overall architecture:**
1. Start with SEARCH_FINDINGS_SUMMARY.md
2. Read the relevant section of ARCHITECTURE_EVENT_NAVIGATION.md
3. Refer to EVENT_SYSTEM_SUMMARY.txt for visual overview

**Implement event-driven navigation:**
1. Read pattern #2 in IMPLEMENTATION_PATTERNS.md
2. Check CODE_MAP.md for exact file locations
3. Modify AddLocationModal.tsx and LocationsSection.tsx
4. Reference the complete example at the end of IMPLEMENTATION_PATTERNS.md

**Debug event handling:**
1. Review the debugging section in IMPLEMENTATION_PATTERNS.md
2. Check the event detection patterns in ARCHITECTURE_EVENT_NAVIGATION.md
3. Use the code in EVENT_SYSTEM_SUMMARY.txt to trace the flow

**Find a specific file:**
1. Use CODE_MAP.md - it has all files with line numbers
2. Cross-reference with absolute paths for copy-paste

**Understand event flow:**
1. Look at flow diagrams in SEARCH_FINDINGS_SUMMARY.md
2. Read section 1 of ARCHITECTURE_EVENT_NAVIGATION.md
3. Review EVENT_SYSTEM_SUMMARY.txt diagrams

---

## Key Findings Summary

### Where Events Happen
- **Reception:** `/packages/ts-client/src/client.ts` - SpacedriveClient.subscribe()
- **Distribution:** client.on('spacedrive-event', handler)
- **Handling:** useNormalizedCache (auto) or useEvent (manual)

### Event Types for Locations
1. **LocationAdded** - Legacy, simple: { location_id, library_id, path }
2. **ResourceChanged** - Modern, complete: { resource_type: "location", resource: {...} }

### Navigation
- **Router:** React Router with useNavigate hook
- **Route:** `/location/:locationId`
- **Used in:** LocationsSection, LocationsGroup, ExplorerView

### Available Listeners
1. **SimpleEventEmitter** - Core: on(), off(), emit(), once()
2. **useEvent** - Hook: useEvent(eventType, handler)
3. **useAllEvents** - Hook: Listen to all events
4. **useNormalizedCache** - Auto: Handles ResourceChanged automatically

---

## File Locations

All absolute paths for reference:

```
Core Event System:
/Users/jamespine/Projects/spacedrive/packages/ts-client/src/client.ts
/Users/jamespine/Projects/spacedrive/packages/ts-client/src/event-filter.ts
/Users/jamespine/Projects/spacedrive/packages/ts-client/src/generated/types.ts

Event Hooks:
/Users/jamespine/Projects/spacedrive/packages/interface/src/hooks/useEvent.ts
/Users/jamespine/Projects/spacedrive/packages/ts-client/src/hooks/useNormalizedCache.ts

Location Components:
/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/AddLocationModal.tsx
/Users/jamespine/Projects/spacedrive/packages/interface/src/components/Explorer/components/LocationsSection.tsx
/Users/jamespine/Projects/spacedrive/packages/interface/src/components/SpacesSidebar/LocationsGroup.tsx

Router:
/Users/jamespine/Projects/spacedrive/packages/interface/src/router.tsx
```

---

## What Each Document Contains

| Document | Size | Focus | Best For |
|----------|------|-------|----------|
| SEARCH_FINDINGS_SUMMARY | 9.9K | High-level overview | Quick understanding |
| ARCHITECTURE_EVENT_NAVIGATION | 14K | Complete system design | Deep learning |
| IMPLEMENTATION_PATTERNS | 11K | Practical code | Building features |
| CODE_MAP | 12K | File references | Finding code |
| EVENT_SYSTEM_SUMMARY | 15K | Visual guide | Quick lookup |
| DOCUMENTATION_INDEX | This | Navigation guide | Getting started |

**Total Documentation:** ~62 KB of comprehensive reference material

---

## Implementation Checklist

Ready to implement? Here's the order:

- [ ] Read SEARCH_FINDINGS_SUMMARY.md (5 min)
- [ ] Read EVENT_SYSTEM_SUMMARY.txt (5 min)
- [ ] Choose pattern from IMPLEMENTATION_PATTERNS.md (2 min)
- [ ] Reference CODE_MAP.md for exact locations (during coding)
- [ ] Read ARCHITECTURE_EVENT_NAVIGATION.md section 6 for details (10 min)
- [ ] Implement the pattern (30-60 min)
- [ ] Test and debug using Debugging section (5-10 min)

---

## Important Notes

### Event Detection
Always use **discriminated union** pattern:
```typescript
if ('LocationAdded' in event) { }  // CORRECT
if (event.type === 'LocationAdded') { }  // WRONG
```

### Resource Type Matching
```typescript
event.ResourceChanged.resource_type === 'location'  // For locations
```

### Listener Cleanup
```typescript
useEffect(() => {
  const unsubscribe = client.on(...);
  return () => unsubscribe?.();  // Don't forget!
}, []);
```

### Auto-Update Pattern
Use `isGlobalList: true` for auto-appending new items:
```typescript
useNormalizedCache({
  wireMethod: 'query:locations.list',
  resourceType: 'location',
  isGlobalList: true,  // Auto-appends new locations
})
```

---

## Questions?

All questions should be answerable by these documents:

1. **Event structure?** → EVENT_SYSTEM_SUMMARY.txt section 2
2. **How to listen?** → IMPLEMENTATION_PATTERNS.md patterns 1 & 4
3. **Where's the code?** → CODE_MAP.md
4. **What's the flow?** → ARCHITECTURE_EVENT_NAVIGATION.md section 4
5. **How to navigate?** → ARCHITECTURE_EVENT_NAVIGATION.md section 5
6. **Complete example?** → IMPLEMENTATION_PATTERNS.md section 9

---

## Version Information

- **Created:** 2025-11-13
- **Based on:** Spacedrive codebase at commit 5a57b0198
- **Relevant packages:** @sd/interface, @sd/ts-client
- **React Router version:** 6.x
- **Event system:** SimpleEventEmitter + TanStack Query

---

## Next Steps

1. Choose your implementation approach (see IMPLEMENTATION_PATTERNS.md)
2. Find exact file locations (see CODE_MAP.md)
3. Review the architecture (see ARCHITECTURE_EVENT_NAVIGATION.md)
4. Implement the code (see IMPLEMENTATION_PATTERNS.md)
5. Test using debugging guide (see IMPLEMENTATION_PATTERNS.md)

All information needed is in these 5 documents. Good luck!
