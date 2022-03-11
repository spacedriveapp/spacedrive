<p align="center">
  <a href="#">
    
  </a>
  <p align="center">
   <img width="100" height="100" src="./apps/desktop/src/assets/images/spacedrive_logo.png" alt="Logo">
  </p>
  <h1 align="center"><b><abbr title="Spacky">Space</abbr>drive</b></h1>
  <p align="center">
   The universal file manager.
    <br />
    <a href="https://spacedrive.co"><strong>spacedrive.app »</strong></a>
    <br />
    <br />
    <b>Download for </b>
    <a href="">macOS</a>
    ·
    <a href="">Windows</a>
    ·
    <a href="">Linux</a>
    ·
    iOS
    ·
    watchOS
    ·
    Android

  </p>
</p>
Spacedrive is an open source, cross platform file manager app. Powered by a secure, virtual distributed filesystem (<a href="#what-is-vdfs">VDFS</a>) written in Rust. <a href="https://spacedrive.app"><strong>Learn more »</strong></a>
<br />
<br />
The model for data storage today is; subscribe to a cloud provider and get storage space. With it come handy tools for organization (photo albums, shared folders etc.). The problem comes once you outgrow that space and the tools aren't so handy anymore. This data doesn't flow between OS platforms and across competing cloud services ...and that's not to mention privacy.
<br />
<br />
Spacedrive brings those tools out of the cloud and onto your devices. To organize, encode, encrypt, share, and preserve the data that defines you, at unlimited scale.
<br />
<br />
<img src="./apps/desktop/src/assets/images/spacedrive_screenshot_2.jpg" alt="Logo">

# Features
> NOTE: Spacedrive is under active development, most of the listed features are still experimental and subject to change.

**Complete:** *(in testing)*
- **[File discovery](#)** - Scan devices, drives and cloud accounts to build a virtual, realtime updating, "yellow pages" directory of all files with metadata.
- **[Preview generation](#)** - Auto generate lower resolution stand-ins for image and video.
- **[Statistics](#)** - Statistics such as total capacity, index size, preview media size, free space etc.
  
**In progress:**
- **[File Explorer](#)** - Browse online/offline storage locations, view files with metadata, perform basic crud. 
- **[Realtime synchronization](#)** - Database synchronizes in realtime between devices, prioritizing peer-to-peer LAN connections (WiFi sync).
  
**To be developed:**
- **[Photos](#)** - Photo and video albums similar to Apple/Google photos, but owned by you and infinite in size.
- **[Search](#)** - Search deep into your filesystem, including offline devices, with a custom keybind.
- **[Cloud compatibility](#)** - Index & backup to Apple Photos, Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.
- **[Tags](#)** - Define routines on custom tags to automate workflows, easily tag files individually, in bulk and automatically via rules.
- **[Encrypted vault(s)](#)** - Effortlessly manage & encrypt sensitive files, built on top of VeraCrypt. Encrypt individual files or create flexible-size vaults.
- **[Key manager](#)** - View, mount, dismount and hide keys. Mounted keys automatically unlock respective areas of your filesystem.
- **[Redundancy](#)** - Ensure a specific amount of copies exist for your important data, discover at-risk files and monitor device/drive health.
- **[Timeline](#)** - View a linear timeline of content, travel to any time and see media represented visually.
- **[Extensions](#)** - Build tools on top of Spacedrive, extend functionality and integrate third party services. Extension directory on [spacedrive.app/extensions](#).
- **[Media encoder](#)** - Encode video and audio into various formats, use Tags to automate.
- **[Workers](#)** - Utilize the compute power of your devices in unison to encode and perform tasks at insane speeds.
- **[Self host](#)** - Spacedrive can be deployed as a service, behaving as just another device powering your personal cloud.
- **[Spacedrive Cloud](#)** - We'll host an always-on cloud device for you, with pay-as-you-go plans for storage.
<!-- - **Spaces** - A collection of files organized visually and shareable as public web pages with a Spacedrive account. -->
<!-- - **Jobs** - Each task a client performs, a body of work we refer to as a "job", is logged and reversible. -->

# Motivation
We depend on cloud services like Google Photos and iCloud, but are locked in with a limited capacity. Many of us have multiple cloud accounts, drives that aren’t backed up and data at risk of loss. But why be tied to any one cloud provider? Photo albums shouldn’t be suck in a device ecosystem, or harvested for advertising data. It should be OS agnostic, permanent and personally owned.

Open source technology is the solution to this, Spacedrive is a universal experience to manage files, across all platforms, devices and clouds. 

# What is VDFS?
A VDFS (virtual distributed filesystem) is a concept first outlined in a UC Berkeley [paper](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2018/EECS-2018-29.pdf) by Haoyuan Li. Simplified, it can be thought of to provide a single UNIX-like interface to a virtualized filesystem above a variety of storage layers. Due to being distributed in nature it has infinite expansion potential, while maintaining a consistent API. This paper describes its use for cloud computing, however the underlying concepts can be translated to open consumer software. Spacedrive is an alternate implementation 

# Architecture
This project is using what I'm calling the **"PRRTT"** stack (Prisma, Rust, React, TypeScript, Tauri). 
- Prisma on the front-end? 🤯 Made possible thanks to [prisma-client-rust](), developed by [Brendonovich](). Gives us access to the powerful migration CLI in development, along with the Prisma syntax for our schema. The application bundles with the Prisma query engine and codegen for a beautiful Rust API. Our lightweight migration runner is custom built for a desktop app context.
- Tauri allows us to create a pure Rust native OS webview, without the overhead of your average Electron app. This brings the bundle size and average memory usage down dramatically. It also contributes to a more native feel, especially on macOS due to Safari's close integration with the OS. 
- ...

Spacedrive's core (`sdcorelib`) is written in pure Rust, using the Tauri framework to embed a React app in a native browser window for UI. The mobile app is React Native, with `sdcorelib` embedded as a native binary. 

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


## Developer Info

- Rust version: **1.58.1**
- Node version: **17**

Install instructions: 
- `$ git clone https://github.com/jamiepine/spacedrive`
- `$ cd spacedrive`
- `$ yarn`
- `$ yarn desktop dev`
