# Welcome to the Spacedrive Contributing Guide

Thank you for investing your time in contributing to our project!

Please read our [Code of Conduct](./CODE_OF_CONDUCT.md) to keep our community approachable and respectable.

This guide will provide an overview of the contribution workflow, including opening an issue, creating a pull request (PR), and the review and merge process.

## New Contributor Guide

To familiarize yourself with the project, please read the [README](README.md). Here are some resources to help you get started with open-source contributions:

- [Finding ways to contribute to open-source on GitHub](https://docs.github.com/en/get-started/exploring-projects-on-github/finding-ways-to-contribute-to-open-source-on-github)
- [Setting up Git](https://docs.github.com/en/get-started/quickstart/set-up-git)
- [GitHub flow](https://docs.github.com/en/get-started/quickstart/github-flow)
- [Collaborating with pull requests](https://docs.github.com/en/github/collaborating-with-pull-requests)
- [Getting started with Tauri](https://tauri.app/v1/guides/getting-started/prerequisites)
- [pnpm CLI](https://pnpm.io/pnpm-cli)

## Getting Started

### Issues

#### Creating a New Issue

If you come across an issue or have a feature request for Spacedrive, please [search if a related issue has already been reported](https://docs.github.com/en/github/searching-for-information-on-github/searching-on-github/searching-issues-and-pull-requests#search-by-the-title-body-or-comments). If no relevant issue exists, you can open a new issue using the appropriate [issue form](https://github.com/spacedriveapp/spacedrive/issues/new/choose).

#### Solving an Issue

To find an issue that interests you, you can browse through our [existing issues](https://github.com/spacedriveapp/spacedrive/issues) and use the available `labels` to narrow down your search (See [Labels](https://github.com/spacedriveapp/spacedrive/labels) for more information). As a general rule, if you find an issue you want to work on, you are welcome to open a PR with a fix.

### Making Changes

#### Making Changes Locally

This project uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation). Make sure you have them installed before proceeding.

To make changes locally, follow these steps:

1. Clone the repository: `git clone https://github.com/spacedriveapp/spacedrive`
2. Navigate to the project directory: `cd spacedrive`
3. Configure your system environment for Spacedrive development
   1. For Linux users, run: `./scripts/setup.sh`
      > This [script](https://github.com/spacedriveapp/spacedrive/blob/main/scripts/setup.sh#L133) will check if Rust and pnpm are installed then proceed to install Clang, NASM, LLVM, libvips, Gstreamer's Plugins, FFmpeg, Perl, [Tauri essentials](https://tauri.app/v1/guides/getting-started/prerequisites/#setting-up-linux) and any other required dependencies for Spacedrive to build.
   2. For macOS users, run: `./scripts/setup.sh`
      > This [script](https://github.com/spacedriveapp/spacedrive/blob/main/scripts/setup.sh#L108) will check if Rust, pnpm and Xcode are installed and proceed to use Homebrew to install NASM, [Tauri essentials](https://tauri.app/v1/guides/getting-started/prerequisites/#setting-up-macos) and install any other required dependencies for Spacedrive to build.
   3. For Windows users, run in PowerShell: `.\scripts\setup.ps1`
      > This [script](https://github.com/spacedriveapp/spacedrive/blob/main/scripts/setup.ps1#L81) will install pnpm, LLVM, FFmpeg, C++ build tools, NASM, Rust + Cargo, Rust tools, Edge Webview 2, Strawberry Perl, [Tauri essentials](https://tauri.app/v1/guides/getting-started/prerequisites/#setting-up-windows) and any other required dependencies for Spacedrive to build.
4. Install dependencies: `pnpm i`
5. Prepare the build: `pnpm prep` (This will run all necessary codegen and build required dependencies)

To quickly run only the desktop app after `prep`, you can use:

- `pnpm tauri dev`

  If necessary, the [webview devtools](https://tauri.app/v1/guides/debugging/application/#webview-console) can be opened by pressing `Ctrl + Shift + I` (Linux and Windows) or `Command + Option + I` (macOS) in the desktop app.

  Also, the react-devtools can be launched using `pnpm dlx react-devtools`.
  However, it must be executed before starting the desktop app for it to connect.

You can download a bundle with sample files to test the app by running:

- `pnpm test-data`

  Only for Linux and macOS (Requires curl and tar).
  The test files will be located in a directory called `test-data` in the root of the spacedrive repo.

To run the web app:

- `pnpm dev:web`

This will start both the server and web interface.
You can launch these individually if you'd prefer:

- `cargo run -p sd-server` (server)
- `pnpm web dev` (web interface)

To run the e2e tests for the web app:

- `pnpm web test:e2e`

If you are developing a new test, you can execute Cypress in interactive mode with:

- `pnpm web test:interactive`

To run the landing page:

- `pnpm landing dev`

If you encounter any issues, ensure that you are using the following versions of Rust, Node and Pnpm:

- Rust version: **1.79**
- Node version: **18.18**
- Pnpm version: **9.4.0**

After cleaning out your build artifacts using `pnpm clean`, `git clean`, or `cargo clean`, it is necessary to re-run the `setup-system` script.

Make sure to read the [guidelines](https://spacedrive.com/docs/developers/prerequisites/guidelines) to ensure that your code follows a similar style to ours.

After you finish making your changes and committed them to your branch, make sure to execute `pnpm autoformat` to fix any style inconsistency in your code.

##### Mobile App

To run the mobile app:

- Install Java JDK <= 17 for Android
  - Java 21 is not compatible: https://github.com/react-native-async-storage/async-storage/issues/1057#issuecomment-1925963956
- Install [Android Studio](https://developer.android.com/studio) for Android and [Xcode](https://apps.apple.com/au/app/xcode/id497799835) for iOS development.
- Run `./scripts/setup.sh mobile`
  - This will set up most of the dependencies required to build the mobile app.
- Make sure you have [NDK 26.1.10909125 and CMake](https://developer.android.com/studio/projects/install-ndk#default-version) installed in Android Studio.
- Run the following commands:
  - `pnpm mobile android` (runs on Android Emulator)
    - In order to have locations working on Android, you must run the following command once the application has been installed for the first time. Otherwise, locations will not work.
      - `adb shell appops set --uid com.spacedrive.app MANAGE_EXTERNAL_STORAGE allow`
    - Run the following commands to access the logs from `sd-core`.
      - `adb shell`
      - Then `run-as com.spacedrive.app` to access the app's directory on device.
      - Run `cd files/logs` and then select the logs with the timestamp of when you ran the app. Ex: `sd.log.2023-11-28`.
        - You can view the logs using `tail -f [log-name]`. Ex: `tail -f sd.log.2023-11-28`.
  - `pnpm mobile ios` (runs on iOS Emulator)
    - `xcrun simctl launch --console booted com.spacedrive.app` allows you to view the console output of the iOS app from `tracing`. However, the application must be built in `debug` mode for this.
  - `pnpm mobile start` (runs the metro bundler only)

### Pull Request

Once you have finished making your changes, create a pull request (PR) to submit them.

- Fill out the "Ready for review" template to help reviewers understand your changes and the purpose of your PR.
- If you are addressing an existing issue, don't forget to [link your PR to the issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue).
- Enable the checkbox to [allow maintainer edits](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/allowing-changes-to-a-pull-request-branch-created-from-a-fork) so that the branch can be updated for merging.
- Once you submit your PR, a team member will review your proposal. They may ask questions or request additional information.
- You may be asked to make changes before the PR can be merged, either through [suggested changes](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/incorporating-feedback-in-your-pull-request) or pull request comments. You can apply suggested changes directly through the UI. For other changes, you can make them in your fork and commit them to your branch.
- As you update your PR and apply changes, mark each conversation as [resolved](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/commenting-on-a-pull-request#resolving-conversations).
- If you run into any merge issues, refer to this [git tutorial](https://lab.github.com/githubtraining/managing-merge-conflicts) to help you resolve merge conflicts and other issues.

### Your PR is Merged!

Congratulations! 🎉🎉 The Spacedrive team thanks you for your contribution! ✨

Once your PR is merged, your changes will be included in the next release of the application.

### Common Errors

#### `xcrun: error: unable to find utility "xctest", not a developer tool or in PATH`

This error occurs when Xcode is not installed or when the Xcode command line tools are not in your `PATH`.

To resolve this issue:

- Install Xcode from the macOS App Store or directly from [here](https://xcodereleases.com/) (requires Apple Account).
- Run `xcode-select -s /Applications/Xcode.app/Contents/Developer`.
  This command will use Xcode's developer tools instead of macOS's default tools.

#### `unable to lookup item 'PlatformPath'`

If you run into this issue, or similar:

```
error: terminated(1): /us/bin/xcrun --sdk macos --show-sdk-platform-path output :
xcrun: error: unable to lookup item 'PlatformPath' from command line tools installation xcrun: error: unable to lookup item 'PlatformPath' in SDK '/Library/Developer /CommandLineTools/SDKs/MacOSX.sdk'
```

Ensure that macOS is fully updated, and that you have Xcode installed (via the app store).

Once that has completed, run `xcode-select --install` in the terminal to install the command line tools. If they are already installed, ensure that you update macOS to the latest version available.

Also ensure that Rosetta is installed, as a few of our dependencies require it. You can install Rosetta with `softwareupdate --install-rosetta --agree-to-license`.

### Translations

Check out the [i18n README](interface/locales/README.md) for more information on how to contribute to translations.

### Credits

This CONTRIBUTING.md file was inspired by the [github/docs CONTRIBUTING.md](https://github.com/github/docs/blob/main/CONTRIBUTING.md) file, and we extend our gratitude to the original author.
