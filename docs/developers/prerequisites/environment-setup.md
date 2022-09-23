---
index: 1
---

# Environment Setup

To get started contributing to Spacedrive, follow this guide carefully.

This project uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation). Ensure you have them installed before continuing.

## Installation 

1. Clone repo: `git clone https://github.com/spacedriveapp/spacedrive`
2. Open directory: `cd spacedrive`
3. For Linux or MacOS users run: `./.github/scripts/setup-system.sh`
   _This will install FFMPEG and any other required dependencies for Spacedrive to build._
4. For Windows users run using PowerShell: `.\.github\scripts\setup-system.ps1`
  _This will install pnpm, LLVM, FFMPEG and any other required dependencies for Spacedrive to build. Ensure you run it like documented above as it expects it is executed from the root of the repository._
5. Install dependencies: `pnpm i`
6. `pnpm prep` - Runs all necessary codegen & builds required dependencies.

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
4. `cd apps/mobile && pnpm i` - This is a separate workspace, you need to do this!
5. `pnpm android` - runs on Android Emulator
6. `pnpm ios` - runs on iOS Emulator
7. `pnpm dev` - For already bundled app - This is only temporarily supported. The final app will require the Spacedrive Rust code which isn't included in Expo Go.

### Troubleshooting

If you are having issues ensure you are using the following versions of Rust and Node:

- Rust version: **1.64.0**
- Node version: **17**