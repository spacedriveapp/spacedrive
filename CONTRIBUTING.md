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
3. For Linux or MacOS users, run: `./.github/scripts/setup-system.sh`
   - This will install FFmpeg and any other required dependencies for Spacedrive to build.
4. For Windows users, run the following command in PowerShell: `.\.github\scripts\setup-system.ps1`
   - This will install pnpm, LLVM, FFmpeg, and any other required dependencies for Spacedrive to build.
5. Install dependencies: `pnpm i`
6. Prepare the build: `pnpm prep` (This will run all necessary codegen and build required dependencies)

To quickly run only the desktop app after `prep`, you can use:

- `pnpm desktop dev`

  If necessary, react-devtools can be launched using `pnpm react-devtools`.
  However, it must be executed before starting the desktop app for it to connect.

To run the web app:

- `cargo run -p server` (runs the server)
- `pnpm web dev` (runs the web embed server)

To run the landing page:

- `pnpm landing dev`

If you encounter any issues, ensure that you are using the following versions of Rust, Node and Pnpm:

- Rust version: **1.70.0**
- Node version: **18**
- Pnpm version: **8.0.0**

After cleaning out your build artifacts using `pnpm clean`, `git clean`, or `cargo clean`, it is necessary to re-run the `setup-system` script.

Make sure to read the [guidelines](https://spacedrive.com/docs/developers/prerequisites/guidelines) to ensure that your code follows a similar style to ours.

##### Mobile App

To run the mobile app:

- Install [Android Studio](https://developer.android.com/studio) for Android and [Xcode](https://apps.apple.com/au/app/xcode/id497799835) for iOS development.
- Run `./.github/scripts/setup-system.sh mobile`
  - This will set up most of the dependencies required to build the mobile app.
- Make sure you have [NDK 23.1.7779620 and CMake](https://developer.android.com/studio/projects/install-ndk#default-version) installed in Android Studio.
- Run the following commands:
  - `pnpm android` (runs on Android Emulator)
  - `pnpm ios` (runs on iOS Emulator)
  - `pnpm start` (runs the metro bundler)

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

- Install Xcode from the Mac App Store.
- Run `xcode-select -s /Applications/Xcode.app/Contents/Developer`.
  This command will use Xcode's developer tools instead of macOS's default tools.

### Credits

This CONTRIBUTING.md file was inspired by the [github/docs CONTRIBUTING.md](https://github.com/github/docs/blob/main/CONTRIBUTING.md) file, and we extend our gratitude to the original author.
