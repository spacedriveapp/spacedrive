<p align="center">
  <a href="#">
    
  </a>
  <p align="center">
   <img width="100" height="100" src="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_logo.png" alt="Logo">
  </p>
  <h1 align="center"><b>Spacedrive</b></h1>
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
Spacedrive is an open source cross-platform file manager, powered by a virtual distributed filesystem (<a href="#what-is-a-vdfs">VDFS</a>) written in Rust. 
<!-- <a href="https://spacedrive.app"><strong>Learn more »</strong></a> -->
<br/>
<br/>
Organize files across many devices in one place. From cloud services to offline hard drives, Spacedrive combines the storage capacity and processing power of your devices into one personal distributed cloud, that is both secure and intuitive to use. 
<br />
<br />
For independent creatives, hoarders and those that want to own their digital footprint. Spacedrive provides a file management experience like no other, and its completely free.
<br />
<br />

> NOTE: Spacedrive is under active development, most of the listed features are still experimental and subject to change. Additionally, most of the links on this page are broken but will be working once the repository is made public. 
> 
<br />

<img src="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_screenshot.jpg" alt="Logo">


# What is a VDFS?
A VDFS (virtual distributed filesystem) is a filesystem designed to work atop a variety of storage layers. It is not restricted to a single machine, with a uniform API to manipulate and access content across many devices. It achieves this by maintaining a virtual index of all storage locations, synchronizing the database between clients in realtime. This implementation also uses [CAS](https://en.wikipedia.org/wiki/Content-addressable_storage) (Content-addressable storage) to uniquely identify files, while keeping record of logical file paths relative to the storage locations. 

The first implementation of a VDFS can be found in this UC Berkeley [paper](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2018/EECS-2018-29.pdf) by Haoyuan Li. This paper describes its use for cloud computing, however the underlying concepts can be translated to open consumer software. 

# Motivation
Many of us have multiple cloud accounts, drives that aren’t backed up and data at risk of loss. We depend on cloud services like Google Photos and iCloud, but are locked in with limited capacity and almost zero interoperability between services and operating systems. Photo albums shouldn’t be suck in a device ecosystem, or harvested for advertising data. They should be OS agnostic, permanent and personally owned. Data we create is our legacy, that will long outlive us—open source technology is the only way to ensure we retain absolute control over the data that defines our lives, at unlimited scale.


# Features
**Complete:** *(in testing)*
- **[File discovery](#)** - Scan devices, drives and cloud accounts to build a directory of all files with metadata.
- **[Preview generation](#)** - Auto generate lower resolution stand-ins for image and video.
- **[Statistics](#)** - Total capacity, index size, preview media size, free space etc.
  
**In progress:**
- **[File Explorer](#)** - Browse online/offline storage locations, view files with metadata, perform basic CRUD. 
- **[Realtime synchronization](#)** - Data index synchronized in realtime between devices, prioritizing peer-to-peer LAN connections (WiFi sync).
  
**To be developed (MVP):**
- **[Photos](#)** - Photo and video albums similar to Apple/Google photos.
- **[Search](#)** - Deep search into your filesystem with a keybind, including offline locations.
- **[Tags](#)** - Define routines on custom tags to automate workflows, easily tag files individually, in bulk and automatically via rules.
- **[Extensions](#)** - Build tools on top of Spacedrive, extend functionality and integrate third party services. Extension directory on [spacedrive.app/extensions](#).
  
**To be developed (Post-MVP):**  
- **[Cloud integration](#)** - Index & backup to Apple Photos, Google Drive, Dropbox, OneDrive & Mega + easy API for the community to add more.
- **[Encrypted vault(s)](#)** - Effortlessly manage & encrypt sensitive files, built on top of VeraCrypt. Encrypt individual files or create flexible-size vaults.
- **[Key manager](#)** - View, mount, dismount and hide keys. Mounted keys automatically unlock respective areas of your filesystem.
- **[Redundancy Goal](#)** - Ensure a specific amount of copies exist for your important data, discover at-risk files and monitor device/drive health.
- **[Timeline](#)** - View a linear timeline of content, travel to any time and see media represented visually.
- **[Media encoder](#)** - Encode video and audio into various formats, use Tags to automate. Built with FFMPEG.
- **[Workers](#)** - Utilize the compute power of your devices in unison to encode and perform tasks at increased speeds.
- **[Spacedrive Cloud](#)** - We'll host an always-on cloud device for you, with pay-as-you-go plans for storage.
- **[Self hosted](#)** - Spacedrive can be deployed as a service, behaving as just another device powering your personal cloud.

# Developer Installation Instructions
This environment uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation). Ensure you have them installed before continuing.

- `$ cargo install prisma-client-rust-cli`
- `$ git clone https://github.com/spacedriveapp/spacedrive`
- `$ cd spacedrive`
- `$ pnpm i`
- `$ pnpm prep` - Runs all necessary codegen & builds required dependencies.

To quickly run only the desktop app after `prep` you can use:
- `$ pnpm desktop dev`

If you are making changes to any TS packages you must run their respective dev environments too, for example: 
- `$ pnpm core dev`
- `$ pnpm ui dev`
  
Or to run everything specific to desktop app development just run:
- `$ pnpm dev`

If you are having issues ensure you are using the following versions of Rust and Node:
- Rust version: **1.58.1**
- Node version: **17**

# Architecture
This project is using what I'm calling the **"PRRTT"** stack (Prisma, Rust, React, TypeScript, Tauri). 
- Prisma on the front-end? 🤯 Made possible thanks to [prisma-client-rust](), developed by [Brendonovich](). Gives us access to the powerful migration CLI in development, along with the Prisma syntax for our schema. The application bundles with the Prisma query engine and codegen for a beautiful Rust API. Our lightweight migration runner is custom built for a desktop app context.
- Tauri allows us to create a pure Rust native OS webview, without the overhead of your average Electron app. This brings the bundle size and average memory usage down dramatically. It also contributes to a more native feel, especially on macOS due to Safari's close integration with the OS. 
- The core (`sdcore`) is written in pure Rust.

## Monorepo structure: 

### Apps:
- `desktop`: A [Tauri](https://nextjs.org) app.
- `mobile`: A [React Native](https://nextjs.org) app.
- `web`: A [Next.js](https://nextjs.org) app.

### Core:
- `core`: The [Rust]() core, referred to internally as `sdcore`. Contains filesystem, database and networking logic. Can be deployed in a variety of host applications.

### Packages:
- `client`: The [TypeScript]() Typescript client handling dataflow between UI and the Rust core.
- `ui`: A [React Native]() / [RNW]() Component library.
- `config`: `eslint` configurations (includes `eslint-config-next`, `eslint-config-prettier` and all `tsconfig.json` configs used throughout the monorepo.
- `macos`: A [Swift]() Native binary for MacOS system extensions.
- `ios`: A [Swift]() Native binary (planned).
- `windows`: A [C#]() Native binary (planned).
- `android`: A [Kotlin]() Native binary (planned).