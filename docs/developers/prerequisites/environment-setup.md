---
index: 1
---

# Environment Setup

To get started contributing to Spacedrive, follow this guide carefully.

## Prerequisites

You'll need the following tools installed:

- [Git](https://git-scm.com/downloads)

The setup script will install the following tools and libraries if not present on your system:

- [Node.js + pnpm](https://pnpm.io/installation)
- [Rust + Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
- [FFmpeg](https://www.ffmpeg.org)
- [OpenSSL](https://www.openssl.org)

The following will be installed on Windows exclusively:

- [vcpkg](https://vcpkg.io) (if VCPKG_ROOT is not set)
- [Visual Studio Build Tools](https://learn.microsoft.com/en-us/visualstudio/install/workload-and-component-ids?view=vs-2022)
  - [Windows 10 SDK (19041)](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022)
  - [C++ Clang Compiler for Windows](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022)
  - [MSVC C++ x64/x86 build tools](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022)
- [Strawberry Perl](https://strawberryperl.com) (if no perl installation is found, **required for build because of OpenSSL**)

<!-- - [Perl Strawberry](https://doc.rust-lang.org/cargo/getting-started/installation.html) -->

The rest of the required tools can be installed by this script.

## Installation

1. **Clone the repository**
   ```shell
   git clone https://github.com/spacedriveapp/spacedrive && cd spacedrive
   ```
2. **Run setup script**

   **Linux and macOS users, run:**

   ```shell
   ./.github/scripts/setup-system.sh
   ```

   This will install FFmpeg and any other required dependencies for Spacedrive to build.

   **Windows users, run:**

   ```powershell
   .\.github\scripts\setup-system.ps1
   ```

   This will install all required dependencies for Spacedrive to build. Ensure you run it as documented above; the script expects to be executed from the root of the repository.

3. **Install dependencies**

   ```shell
   pnpm install
   ```

4. **Run codegen & build required dependencies**

   ```shell
   pnpm prep
   ```

## Running apps

- **Desktop:** `pnpm desktop dev`
- **Landing:** `pnpm landing dev`
- **Server:** `DATA_DIR=/path/to/library cargo run -p sdcore`
- **Web app:** `pnpm web dev`

::: slot note
When changing branches, make sure to run `pnpm prep` command. This ensures all generated code is up to date.
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
  <<<<<<< HEAD
- # Node.js version: **≥17**
- Node version: **17**
  > > > > > > > 3bed836989b6eb5eda8d0045ed10461e1772caa9
