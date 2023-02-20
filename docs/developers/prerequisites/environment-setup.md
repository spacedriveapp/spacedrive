---
index: 1
---

# Environment Setup

To get started contributing to Spacedrive, follow this guide carefully.

This project uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation).

## Installation

1. **Clone repo**
   ```shell
   git clone https://github.com/spacedriveapp/spacedrive && cd spacedrive
   ```
2. **Run setup script**

   **For Linux or MacOS users run:**

   ```shell
   ./.github/scripts/setup-system.sh
   ```

   This will install FFmpeg and any other required dependencies for Spacedrive to build.

   **...or for Windows users run using PowerShell:**

   ```shell
   .\.github\scripts\setup-system.ps1
   ```

   _This will install pnpm, LLVM, FFmpeg and any other required dependencies for Spacedrive to build. Ensure you run it like documented above as it expects it is executed from the root of the repository._

3. **Install dependencies**
   ```shell
   pnpm i
   ```
4. **Run codegen & build required dependencies**
   ```shell
   pnpm prep
   ```

## Running apps

- **Desktop:** `pnpm desktop dev`
- **Landing:** `pnpm landing dev`
- **Server:** `DATA_DIR=/path/to/library cargo run -p sdcore`
- **Webapp:** `pnpm web dev`

::: slot note
When changing branches, make sure to run `pnpm prep` command right after. This ensures all the codegen is up to date.
:::

### Mobile app

To run mobile app

1. Install [Android Studio](https://developer.android.com/studio) for Android and [Xcode](https://apps.apple.com/au/app/xcode/id497799835) for IOS development
2. `./.github/scripts/setup-system.sh mobile`
   _The should setup most of the dependencies for the mobile app to build._
3. You must also ensure you have [NDK 24.0.8215888 and CMake](https://developer.android.com/studio/projects/install-ndk#default-version) in Android Studio
4. `pnpm mobile android` - runs on Android Emulator
5. `pnpm mobile ios` - runs on iOS Emulator

### Troubleshooting

If you are having issues ensure you are using the following versions of Rust and Node:

- Rust version: **1.67.0**
- Node version: **17**
