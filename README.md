<p align="center">
  <a href="#">
    <img src="https://cdn.discordapp.com/attachments/795140682247307277/925649615867498516/CleanShot_2021-12-28_at_23.19.592x.png" alt="Logo">
    
  </a>
  <h2 align="center">Spacedrive</h2>
  <p align="center">
    Your private virtual cloud.
    <br />
    <a href="https://spacedrive.co"><strong>Learn more »</strong></a>
    <br />
    <br />
    <a href="">Discord</a>
    ·
    <a href="https://spacedrive.co">Website</a>
    ·
    <a href="https://github.com/jamiepine/spacedrive/issues">Issues</a>
  </p>
</p>

Spacedrive is a privacy-first, open source, virtual filesystem powered by your devices combined. All the feature-rich benefits of the cloud but hosted, controlled and owned by you. Through a single beautifully designed cross platform interface, you can build a limitless directory of your entire digital life that will stand the test of time.

Out of the box support for photos, movies, albums, audio, VODs, code repositories, social media backups, NFTs, screenshots, saved web pages, links and notes.

## Features
- **File indexing** - Scan your devices, drives, removable storage and cloud accounts to build a virtual "yellow pages" directory of all your data.
- **Realtime synchronization** - Sync database in realtime between devices, securely.
- **Photos** - Photo and video albums similar to Apple/Google photos.
- **Search** - Search your entire index at the press of a keybind, including offline devices, drives and clouds.
- **Cloud integration** - Apple Photos, Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.
- **Encrypted vault(s)** - Effortlessly manage & encrypt sensitive files, built on top of VeraCrypt. Encrypt individual files or create flexible-size vaults.
- **Key manager** - View, mount, dismount and hide keys. Mounted keys automatically unlock respective areas of your filesystem.
- **Smart tags** - Define routines on custom tags to automate workflows, easily tag files individually, in bulk and automatically via rules.
- **Spaces** - A collection of files organized visually and shareable as public web pages with a Spacedrive account.
- **Statistics** - View statistics such as total capacity, index size, preview media size, free space—to name a few.
- **Timeline** - View a linear timeline of content, travel to any time and see media represented visually, including overlapping content.
- **Extensions** - Build tools on top of Spacedrive, extend functionality and integrate third party services. Extension directory on [spacedrive.co/extensions](#).
- **Manage redundancy** - Ensure a specific amount of copies exist for your important data, discover at-risk files and monitor device/drive health.
- **Media encoder** - Encode video and audio into various formats, use Tags to automate.
- **Workers** - Utilize the compute power of your devices in unison to encode and perform tasks at insane speeds.
- **Self host** - Spacedrive can be deployed as a service, behaving as just another device powering your personal cloud.
- **Spacedrive Cloud** - We'll host an always-on cloud device for you, with pay-as-you-go plans for storage.

## Motivation
Independent creatives are the new normal, our data is steadily accumulating in the terabytes but the infrastructure hasn’t caught up. Cloud services like Google Photos and iCloud have great features, beautiful UI—but you’re locked in with a very limited capacity. Many people have multiple cloud accounts, drives that aren’t backed up and collecting dust, data at risk of loss. I don't want to be tied to any one cloud provider; a photo album shouldn’t exist only in only my iCloud account, it should be universal and permanent. I believe open source technology is the solution to this, with incredibly an versatile and secure architecture we can create a uniform experience to provide control over vast amounts of data in many environments. 

<!-- With a cultural boom of independent creatives there is a lack of tools to support the ever increasing amount of data accumulated. Cloud services have great features, but require your content to be *in* the cloud to benefit from them. For most creators a 50GB OBS recording is just not convenient to upload. 

I believe, in the advent of web3, we need to control and own our own data portfolios, not cloud companies. One uniform way to track, organize, back-up, share, encrypt and view an unlimited amount of data, not locking into a single provider and living within their limits.  -->

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