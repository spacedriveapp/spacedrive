# Spacedrive V1 → V2 Migration & Release Strategy

## Current Situation

**V1 (Tauri/TypeScript):**
- Last release: v0.4.3
- ~600k downloads total
- Auto-updater active on macOS/Windows (uses Tauri updater plugin)
- Data location: `~/Library/Application Support/spacedrive/` (macOS)
- 35k GitHub stars on main repo
- Community expects updates through auto-updater

**V2 (Rust/Swift):**
- Complete rewrite, different architecture
- CLI + daemon (Rust)
- Native macOS app (Swift, can connect to daemon but doesn't bundle it yet)
- Native iOS app (Swift, bundles core via FFI)
- No Tauri, no TypeScript, no React
- Libraries: `.sdlibrary/` format (incompatible with V1)

## Critical Challenges

### 1. **Breaking Change in Every Way**
- Different UI (SwiftUI vs React)
- Different data format (`.sdlibrary` vs V1 database)
- Different architecture (embedded core vs daemon)
- No backward compatibility possible

### 2. **Auto-Updater Expectations**
- 600k V1 users expect updates via auto-updater
- V1 updater endpoint: `https://spacedrive.com/api/releases/tauri/{{version}}/{{target}}/{{arch}}`
- If we push v2.0.0 through V1's updater, it will try to replace Tauri app with... nothing (V2 has no Tauri app)

### 3. **Version Numbering Dilemma**
- V1 is at 0.4.3
- V2 deserves to be 2.0.0 (or 1.0.0)
- But jumping from 0.4.3 → 2.0.0 needs careful handling

### 4. **Multiple Release Artifacts**
- CLI/daemon (cross-platform)
- macOS app (.app bundle)
- iOS app (App Store only)
- Eventually: Android, Windows native, Linux native

## Recommended Strategy

### Phase 1: Soft Launch (Week 1)

**Goal:** Get V2 out without breaking V1 users

1. **Merge V2 into main repo** but keep V1 updater **unchanged**
   - Don't push any updates through V1's auto-updater yet
   - This preserves V1 users' current state

2. **Release V2 as opt-in downloads:**
   - GitHub Release tagged `v2.0.0-alpha.1`
   - Manual downloads for early adopters:
     - `spacedrive-cli-{platform}.tar.gz` - CLI + daemon
     - `Spacedrive.app.tar.gz` - macOS app (unsigned for now)
   - iOS app: TestFlight beta (App Store takes time)

3. **Documentation:**
   - Clear README explaining V1 vs V2
   - Migration guide: "V2 uses new `.sdlibrary` format - your V1 libraries won't be migrated yet"
   - Set expectations: V2 is alpha, V1 is stable (no more updates)

4. **Communicate:**
   - Blog post: "Spacedrive V2: A Complete Reimagining"
   - Discord announcement
   - GitHub Discussions post
   - Be transparent: This is a ground-up rewrite, not an update

### Phase 2: CLI/Daemon Releases (Week 2-4)

**Goal:** Establish V2 distribution for power users

1. **Set up GitHub Actions for CLI:**
   ```yaml
   # Build for: macOS (x64/arm64), Linux (x64/arm64), Windows (x64)
   # Artifacts: spacedrive-cli-{os}-{arch}.tar.gz
   # Install: Extract to /usr/local/bin or add to PATH
   ```

2. **Homebrew formula** (macOS/Linux):
   ```ruby
   class Spacedrive < Formula
     desc "Virtual Distributed File System"
     homepage "https://spacedrive.com"
     url "https://github.com/spacedriveapp/spacedrive/releases/v2.0.0-alpha.1/spacedrive-cli-macos-arm64.tar.gz"
     # ...
   end
   ```

3. **Version:** Start at `2.0.0-alpha.1`, increment alpha versions weekly

### Phase 3: Native macOS App (Month 2)

**Goal:** Replace Tauri app with native Swift app

1. **Bundle daemon with macOS app:**
   - App bundle includes `sd-daemon` binary
   - App can launch its own daemon OR connect to user-installed daemon
   - Auto-detect: If daemon running, connect. Else, launch bundled daemon.

2. **Code signing & notarization:**
   - Get Apple Developer cert
   - Sign both app and daemon binary
   - Notarize for Gatekeeper

3. **Distribution:**
   - DMG installer
   - **Separate from V1's update channel** (new identifier: `com.spacedrive.v2`)
   - Sparkle framework for future auto-updates (not Tauri updater)

### Phase 4: V1 Sunset Communication (Month 3)

**Goal:** Gracefully deprecate V1 without breaking users

**Option A: One-Time Migration Prompt (Recommended)**

Create a **final V1 update (v0.4.4)** that:
1. Shows in-app notice: "Spacedrive V2 is available"
2. Offers to:
   - Back up V1 data to `~/Library/Application Support/spacedrive-v1-backup/`
   - Download V2 installer
   - Disable V1's auto-updater (prevent future interruptions)
3. Does **NOT** auto-install V2 (user choice)

**Implementation:**
```rust
// V1's final update: apps/desktop/src/v2-migration.tsx
async function showV2MigrationPrompt() {
  const result = await dialog.confirm({
    title: "Spacedrive V2 Available",
    message: "A ground-up rewrite with improved performance. Your V1 libraries will need to be re-indexed. Back up V1 data and download V2?",
    okLabel: "Download V2",
    cancelLabel: "Stay on V1"
  });

  if (result) {
    await backupV1Data();
    await openURL("https://spacedrive.com/download/v2");
    await disableAutoUpdater();
  }
}
```

**Option B: Parallel Coexistence (Conservative)**

- Leave V1 auto-updater pointing to v0.4.3 forever
- V1 and V2 can coexist (different data dirs, different app bundles)
- Users manually migrate when ready
- Downside: Confusing, users don't know which to use

### Phase 5: iOS App Store (Month 3-4)

1. App Store submission (2-4 week review process)
2. Separate app ID from V1 (if V1 was ever on App Store, which it wasn't)
3. Version: Start at `2.0.0` (App Store guidelines)

### Phase 6: Stable Release (Month 6)

1. After alpha testing, move to **`v2.0.0` stable**
2. Update main README to default to V2
3. V1 becomes "legacy" branch (archive, no more updates)
4. All auto-update channels point to V2

## Version Numbering Plan

```
V1 (Tauri):
├── v0.4.3 (current, last release)
└── v0.4.4 (final, migration prompt) [optional]

V2 (Rust/Swift):
├── v2.0.0-alpha.1 (initial release)
├── v2.0.0-alpha.2
├── v2.0.0-alpha.N
├── v2.0.0-beta.1
├── v2.0.0-beta.N
└── v2.0.0 (stable, public launch)
```

**Rationale for 2.0.0:**
- Skipping 1.0.0 signals this is V2, not V1
- Matches "Version 2.0" in README
- Clear break from V1's 0.x versioning

## Data Migration Strategy

**Current Status:** No migration path exists

**Short-term (Alpha/Beta):**
- V2 uses `.sdlibrary/` format
- Users create fresh libraries
- Document: "V1 libraries not migrated yet - you'll re-index"

**Long-term (Post-2.0.0):**
Potential migration tool:
```bash
# Future: spacedrive migrate-from-v1
sd-cli migrate-v1 ~/Library/Application\ Support/spacedrive/
# Outputs: ~/Documents/Migrated.sdlibrary/
```

Challenges:
- V1 and V2 database schemas completely different
- V1 used Prisma, V2 uses SeaORM
- Locations, tags, file metadata - all different structure

**Realistic timeline:** Post-launch, low priority (most users will re-index)

## CI/CD Strategy

### GitHub Actions Workflows

**.github/workflows/release-cli.yml:**
```yaml
name: Release CLI
on:
  push:
    tags: ['v2.*']
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        arch: [x86_64, aarch64]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p sd-cli
      - run: tar -czf spacedrive-cli-${{ matrix.os }}-${{ matrix.arch }}.tar.gz target/release/sd-cli
      - uses: actions/upload-artifact@v4
```

**.github/workflows/release-macos.yml:**
```yaml
name: Release macOS App
on:
  push:
    tags: ['v2.*']
jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: maxim-lobanov/setup-xcode@v1
      - run: xcodebuild -project apps/macos/Spacedrive.xcodeproj -scheme Spacedrive archive
      - run: create-dmg Spacedrive.app
      # TODO: Code signing with secrets.APPLE_CERTIFICATE
```

### Auto-Update Architecture (V2)

**Don't use Tauri updater** (we're not using Tauri)

**For macOS app, use Sparkle:**
```swift
// macOS app uses Sparkle framework
import Sparkle

let updater = SPUStandardUpdaterController(
    startingUpdater: true,
    updaterDelegate: nil,
    userDriverDelegate: nil
)

// Appcast URL: https://spacedrive.com/api/releases/v2/appcast.xml
```

**For CLI/daemon:**
- Manual updates via `sd-cli update`
- Or package managers (Homebrew, apt, etc.)

## Repository Structure After Merge

```
spacedriveapp/spacedrive/
├── main branch (V2 code)
├── v1 branch (archived V1 code, no more development)
├── releases/
│   ├── v0.4.3 (V1 final stable)
│   ├── v0.4.4 (V1 migration prompt, optional)
│   ├── v2.0.0-alpha.1
│   └── v2.0.0-alpha.N
└── README.md (updated for V2, link to V1 branch)
```

## Communication Plan

### Day of Merge:

1. **Blog Post:** "Spacedrive V2: The Complete Rewrite"
   - Why we rewrote from scratch
   - What's new (SdPath, sync, performance)
   - Download V2 alpha
   - V1 remains available, no forced upgrades

2. **GitHub Release Notes (v2.0.0-alpha.1):**
   ```markdown
   # Spacedrive V2.0.0 Alpha 1

   This is a **complete rewrite** of Spacedrive with a new architecture.

   **NOT compatible with V1 libraries.** You'll need to create new libraries.

   Downloads:
   - CLI: [Download for macOS/Linux/Windows]
   - macOS app: [Download .dmg] (unsigned)
   - iOS: Coming to TestFlight

   V1 (Tauri app) remains at v0.4.3 and will receive no more updates.
   ```

3. **Discord Announcement:**
   - Pin in #announcements
   - FAQ: "Will my V1 data migrate?" → "Not yet, create fresh libraries"

4. **README Update:**
   - Prominent V2 alpha notice at top
   - "Looking for V1? See the [v1 branch](https://github.com/spacedriveapp/spacedrive/tree/v1)"

### Ongoing:

- Weekly alpha releases
- Changelog for each release
- Community feedback in Discord/Discussions

## Risk Mitigation

### Risk: Users angry about no migration
**Mitigation:**
- Clear communication upfront
- V1 remains available
- Promise future migration tool

### Risk: V1 auto-updater breaks
**Mitigation:**
- Don't touch V1's updater endpoint
- Keep v0.4.3 as perpetual "latest" for V1

### Risk: V2 alpha is buggy, scares users
**Mitigation:**
- Label clearly as "alpha"
- V1 remains stable option
- Rapid iteration on V2 (weekly alphas)

### Risk: Code signing delays macOS release
**Mitigation:**
- Release unsigned .app first (users can right-click → Open)
- Get code signing sorted in parallel
- Re-release signed version as v2.0.0-alpha.2

## Timeline

| Week | Milestone |
|------|-----------|
| 1 | Merge V2 to main, tag v2.0.0-alpha.1, blog post |
| 2-3 | Set up CLI CI/CD, Homebrew formula |
| 4-6 | Weekly alpha releases, bug fixes |
| 6-8 | macOS app code signing, DMG installer |
| 8-10 | iOS TestFlight beta |
| 10-12 | Beta releases (v2.0.0-beta.N) |
| 12+ | Stable v2.0.0 release |

## Next Steps (This Weekend)

1. **Prepare merge:**
   - Create `v1` branch from current main
   - Update V2's README
   - Write blog post draft

2. **Set up releases:**
   - GitHub Actions for CLI builds
   - Create v2.0.0-alpha.1 release (manual upload for now)

3. **Test locally:**
   - Verify V1 and V2 can coexist
   - Different data dirs: `~/Library/Application Support/spacedrive/` vs `~/Documents/*.sdlibrary/`

4. **Communicate:**
   - Draft Discord announcement
   - Prepare FAQ for common questions

## Conclusion

**Don't hijack V1's auto-updater.** V2 is a different app. Treat it as a parallel product initially, then gracefully deprecate V1 once V2 is stable.

The 600k V1 users deserve a smooth transition, not a forced breaking upgrade. Give them choice, clear communication, and time to migrate.
