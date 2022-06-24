# Welcome to the Spacedrive contributing guide

Thank you for investing your time in contributing to our project!

Read our [Code of Conduct](./CODE_OF_CONDUCT.md) to keep our community approachable and respectable.

In this guide you will get an overview of the contribution workflow from opening an issue, creating a PR, reviewing, and merging the PR.

## New contributor guide

To get an overview of the project, read the [README](README.md). Here are some resources to help you get started with open source contributions:

- [Finding ways to contribute to open source on GitHub](https://docs.github.com/en/get-started/exploring-projects-on-github/finding-ways-to-contribute-to-open-source-on-github)
- [Set up Git](https://docs.github.com/en/get-started/quickstart/set-up-git)
- [GitHub flow](https://docs.github.com/en/get-started/quickstart/github-flow)
- [Collaborating with pull requests](https://docs.github.com/en/github/collaborating-with-pull-requests)
- [Getting started with Tauri](https://tauri.app/v1/guides/getting-started/prerequisites)
- [pnpm CLI](https://pnpm.io/pnpm-cli)

## Getting started

### Issues

#### Create a new issue

If you find an issue with the repository or have a feature request with Spacedrive, [search if an issue already exists](https://docs.github.com/en/github/searching-for-information-on-github/searching-on-github/searching-issues-and-pull-requests#search-by-the-title-body-or-comments). If a related issue doesn't exist, you can open a new issue using a relevant [issue form](https://github.com/spacedriveapp/spacedrive/issues/new/choose).

#### Solve an issue

Scan through our [existing issues](https://github.com/spacedriveapp/spacedrive/issues) to find one that interests you. You can narrow down the search using `labels` as filters. See [Labels](https://github.com/spacedriveapp/spacedrive/labels) for more information. As a general rule. If you find an issue to work on, you are welcome to open a PR with a fix.

### Make Changes

#### Make changes locally

This project uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation). Ensure you have them installed before continuing.

> Note: MacOS M1 users should choose the customize option in the rustup init script and enter `x86_64-apple-darwin` as the default host triple instead of the default `aarch64-apple-darwin`

- `$ git clone https://github.com/spacedriveapp/spacedrive`
- `$ cd spacedrive`
- For Linux or MacOS users run: `./.github/scripts/setup-system.sh`
  - This will install FFMPEG and any other required dependencies for Spacedrive to build.
- `$ pnpm i`
- `$ pnpm prep` - Runs all necessary codegen & builds required dependencies.

To quickly run only the desktop app after `prep` you can use:

- `$ pnpm desktop dev`

To run the landing page

- `$ pnpm web dev` - runs the web app for the embed
- `$ pnpm landing dev`

If you are having issues ensure you are using the following versions of Rust and Node:

- Rust version: **1.60.0**
- Node version: **17**

### Pull Request

When you're finished with the changes, create a pull request, also known as a PR.

- Fill the "Ready for review" template so that we can review your PR. This template helps reviewers understand your changes as well as the purpose of your pull request.
- Don't forget to [link PR to issue](https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue) if you are solving one.
- Enable the checkbox to [allow maintainer edits](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/allowing-changes-to-a-pull-request-branch-created-from-a-fork) so the branch can be updated for a merge.
  Once you submit your PR, a team member will review your proposal. We may ask questions or request for additional information.
- We may ask for changes to be made before a PR can be merged, either using [suggested changes](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/incorporating-feedback-in-your-pull-request) or pull request comments. You can apply suggested changes directly through the UI. You can make any other changes in your fork, then commit them to your branch.
- As you update your PR and apply changes, mark each conversation as [resolved](https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/commenting-on-a-pull-request#resolving-conversations).
- If you run into any merge issues, checkout this [git tutorial](https://lab.github.com/githubtraining/managing-merge-conflicts) to help you resolve merge conflicts and other issues.

### Your PR is merged!

Congratulations :tada::tada: The Spacedrive team thanks you :sparkles:.

Once your PR is merged, your contributions will be included in the next release of the application.

### Credits

This CONTRIBUTING.md file was modelled after the [github/docs CONTRIBUTING.md](https://github.com/github/docs/blob/main/CONTRIBUTING.md) file, and we thank the original author.
