<p align="center">
  <a href="#">
    
  </a>
  <p align="center">
   <img width="100" height="100" src="./apps/desktop/src/assets/images/spacedrive_logo.png" alt="Logo">
  </p>
  <h2 align="center"><b>Spacedrive</b></h2>
  <p align="center">
   The universal file explorer.
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
Spacedrive is an open source, cross platform file manager. Powered by a secure, distributed virtual filesystem (VFS) written in Rust. <a href="https://spacedrive.co"><strong>Learn more »</strong></a>
<!-- <br />
<br />
By uniting your devices, clouds and drives into one synchronized filesystem, Spacedrive becomes a private cloud— -->
<br />
<br />
The model for data storage today is; subscribe to a cloud provider and get storage space. With that space come handy tools for organization (photo albums, shared folders etc.). The problem comes once you outgrow that space and the tools aren't so handy anymore. Your data doesn't translate well across OS platforms and between competing cloud services—and that's not to mention privacy.
<br />
<br />
Spacedrive brings those tools out of the cloud and onto your devices. To organize, encode, encrypt, share, and preserve the data that defines you, at unlimited scale.

<!-- The only thing cloud providers provide you is the storage real-estate, you define your storage archive with  -->
<!-- <br />
<br />
Albums in the Apple's Photos app, while beautifully designed, are exclusive and restricted to the Apple Photos app, they have to be present in a single library and can not be split or divided between storage locations or competing cloud services. Apple's tools to organize photos are exclusive to Apple's cloud service which is limited. 
<br />
<br />
- Spacedrive will index native filesystems.
- Search offline storage, track duplicates (across all devices drives and clouds),
- create photo albums, encode video, encrypt sensitive data, automate routines. 
- Organize as if everything was in one place.
<!--
For many independent creatives there is no one cloud or solution for the growing amount of rich media created daily — these are memories, creations, archives.  --> 
<!-- Spacedrive is a file manger that combines the storage capacity and power of all your devices into one synchronized <a>virtual filesystem</a>, with or without the cloud. 
-->
<br />
<br />
<img src="./apps/desktop/src/assets/images/spacedrive_screenshot_2.jpg" alt="Logo">

<!-- Spacedrive is an open source virtual filesystem, a personal cloud powered by your everyday devices. Feature-rich benefits of the cloud, only its owned and hosted by you with security, privacy and ownership as a foundation. Spacedrive makes it possible to create a limitless directory of your digital life that will stand the test of time.

For each client you install, you'll have another node in your personal network. They all share a single encrypted database and work as a team to perform tasks. Prioritizing peer-to-peer LAN connections but always using end-to-end encryption to synchronize in realtime.

As for UI, it has everything you'd expect from a file explorer and more; a native photo viewer, video and audio player. But also *specific* support for VODs, git repositories, social media backups, NFTs, screenshots, webpage snapshots, links, notes and more. Community extensions can add support for different filetypes and tailored file viewers. -->

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
<!-- Independent creatives define society today, we've produced terabytes of rich data — entire lifetimes digitized, but we don't own or control most of it. The tools to organize, backup and share your files belong to cloud services like Google Drive and iCloud. 

The albums you create— -->



We depend on cloud services like Google Photos and iCloud, but are locked in with a limited capacity. Many of us have multiple cloud accounts, drives that aren’t backed up and data at risk of loss. But why should we be tied to any one cloud provider? Photo albums shouldn’t be suck in a device ecosystem, or harvested for advertising data. It should be OS agnostic, permanent and personally owned.

We believe open source technology is the solution to this, Spacedrive is a universal experience to manage files, across all platforms, devices and clouds. 

<!-- Spacedrive is an app that gives you the tools of the cloud, without the cloud. -->

<!-- With a cultural boom of independent creatives there is a lack of tools to support the ever increasing amount of data accumulated. Cloud services have great features, but require your content to be *in* the cloud to benefit from them. For most creators a 50GB OBS recording is just not convenient to upload. 

I believe, in the advent of web3, we need to control and own our own data portfolios, not cloud companies. One uniform way to track, organize, back-up, share, encrypt and view an unlimited amount of data, not locking into a single provider and living within their limits.  -->

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
