# Spacedrive
<!-- Spacedrive is a cross-platform file manager that brings the convenience of the cloud to your own private network. Designed specifically for independent creatives. -->

Spacedrive is a privacy-first, open source, virtual filesystem—powered by your devices in unison. The benefits of the cloud, owned and controlled by you through a single directory representing your entire digital life, synchronized in realtime between devices.

<!-- Streamline ingesting and sorting media such as screenshots, photos, OBS recordings, code repositories—virtually anything, even NFTs. -->

## Features
- **File indexing** - scan your devices, drives, removable storage and cloud accounts to build a virtual "yellow pages" directory of your data.
- **Realtime synchronization** - sync data between devices securely in realtime.
- **Photos** - Beautiful photo and video albums similar to Apple/Google Photos
- **Search** - Search your entire index at the press of a keybind, including offline devices, drives and clouds.
- **Cloud integration** - Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.
- **Encrypted vault(s)** - Effortlessly manage & encrypt sensitive files, built on top of VeraCrypt. Encrypt individual files or create flexible-size vaults.
- **Key manager** - View, mount, dismount and hide keys. Mounted keys automatically unlock respective areas of your filesystem.
- **Smart tags** - Define routines on tags to automate workflows.
- **Extensions** - Build tools on top of Spacedrive, extend functionality and integrate third parties. Extension directory on spacedrive.co/extensions
- **Manage redundancy** - Ensure a specific amount of copies exist for your important data, discover at-risk files and monitor device/drive health.
- **Media encoder** - Encode video and audio into various formats, use Tags to automate.
- **Self host** - Spacedrive can run in the cloud, acting as just another one of your devices that is always on.
- **SpaceCloud** - We'll host an always-on cloud device for you, with pay-as-you-go plans.

## Motivation
With a cultural boom of independent creatives there is a lack of tools to support the ever increasing amount of data accumulated. Cloud services have great features, but require your content to be *in* the cloud to benefit from them. For most creators a 50GB OBS recording is just not convenient to upload. 

I believe, in the advent of web3, we need to control and own our own data portfolios, not cloud companies. One uniform way to track, organize, back-up, share, encrypt and view an unlimited amount of data, not locking into a single provider and living within their limits. 

## Architecture
Spacedrive's core is written in pure Rust, with a web based Typescript React UI and native binaries to support additional functionality per platform.

## Apps
- `desktop`: a [Tauri](https://nextjs.org) app
- `mobile`: a [React Native](https://nextjs.org) app
- `web`: another [Next.js](https://nextjs.org) app
- `docs`: a [Next.js](https://nextjs.org) app
  
## Packages
All TypeScript packages are compiled automatically using Turborepo.
- `core`: the [Rust]() core logic library, referred to internally as `sdcorelib`
- `state`: the [TypeScript]() core logic library
- `ui`: a [React Native]() / [RNW]() component library
- `config`: `eslint` configurations (includes `eslint-config-next`, `eslint-config-prettier` and all `tsconfig.json` configs used throughout the monorepo
- `native-macos`: a [Swift]() native binary
- `native-ios`: a [Swift]() native binary
- `native-windows`: a [C#]() native binary
- `native-android`: a [Kotlin]() native binary


## 