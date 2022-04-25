<p align="center">
  <a href="#">
    
  </a>
  <p align="center">
   <img width="150" height="150" src="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_icon.png" alt="Logo">
  </p>
  <h1 align="center"><b>Spacedrive</b></h1>
  <p align="center">
   The universal file manager.
    <br />
    <a href="https://spacedrive.app"><strong>spacedrive.app »</strong></a>
    <br />
    <br />
    <b>Download for </b>
    macOS
    ·
    Windows
    ·
    Linux
    ·
    iOS
    ·
    watchOS
    ·
    Android
    <br />
    <i>~ links will be added once release available ~</i>
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
<p align="center">
  <img src="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_screenshot.jpg" alt="Logo">
  <br />
  <br />
  <a href="https://discord.gg/gTaF2Z44f5">
    <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
  </a>
  <a href="https://twitter.com/spacedriveapp">
    <img src="https://img.shields.io/badge/Twitter-00acee?logo=twitter&logoColor=white" />
  </a>
  <a href="https://instagram.com/spacedriveapp">
    <img src="https://img.shields.io/badge/Instagram-E4405F?logo=instagram&logoColor=white" />
  </a>
  <img src="https://img.shields.io/static/v1?label=Licence&message=GNU%20v3&color=000" />
  <img src="https://img.shields.io/static/v1?label=Bundled%20Size&message=16.3MB&color=0974B4" />
  <img src="https://img.shields.io/static/v1?label=Stage&message=Alpha&color=2BB4AB" />
  <br />
</p>

> NOTE: Spacedrive is under active development, most of the listed features are still experimental and subject to change. Additionally, most of the links on this page are broken but will be working once the repository is made public.

# What is a VDFS?

A VDFS (virtual distributed filesystem) is a filesystem designed to work atop a variety of storage layers. It is not restricted to a single machine, with a uniform API to manipulate and access content across many devices. It achieves this by maintaining a virtual index of all storage locations, synchronizing the database between clients in realtime. This implementation also uses [CAS](https://en.wikipedia.org/wiki/Content-addressable_storage) (Content-addressable storage) to uniquely identify files, while keeping record of logical file paths relative to the storage locations.

The first implementation of a VDFS can be found in this UC Berkeley [paper](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2018/EECS-2018-29.pdf) by Haoyuan Li. This paper describes its use for cloud computing, however the underlying concepts can be translated to open consumer software.

# Motivation

Many of us have multiple cloud accounts, drives that aren’t backed up and data at risk of loss. We depend on cloud services like Google Photos and iCloud, but are locked in with limited capacity and almost zero interoperability between services and operating systems. Photo albums shouldn’t be suck in a device ecosystem, or harvested for advertising data. They should be OS agnostic, permanent and personally owned. Data we create is our legacy, that will long outlive us—open source technology is the only way to ensure we retain absolute control over the data that defines our lives, at unlimited scale.

# Features
Feature list moved to the [roadmap](docs/product/roadmap.md).

# Developer Installation Instructions

This environment uses [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) and [pnpm](https://pnpm.io/installation). Ensure you have them installed before continuing.

- `$ git clone https://github.com/spacedriveapp/spacedrive`
- IMPORTANT: _Install [FFMPEG](https://www.ffmpeg.org/download.html) if you don't have it already_
- `$ cd spacedrive`
- `$ pnpm i`
- `$ pnpm prep` - Runs all necessary codegen & builds required dependencies.

To quickly run only the desktop app after `prep` you can use:

- `$ pnpm desktop dev`

To run the landing page

- `$ pnpm web dev` - runs the web app for the embed
- `$ pnpm landing dev`

If you are having issues ensure you are using the following versions of Rust and Node:

- Rust version: **1.58.1**
- Node version: **17**

# Architecture

This project is using what I'm calling the **"PRRTT"** stack (Prisma, Rust, React, TypeScript, Tauri).

- Prisma on the front-end? 🤯 Made possible thanks to [prisma-client-rust](https://github.com/brendonovich/prisma-client-rust), developed by [Brendonovich](https://github.com/brendonovich). Gives us access to the powerful migration CLI in development, along with the Prisma syntax for our schema. The application bundles with the Prisma query engine and codegen for a beautiful Rust API. Our lightweight migration runner is custom built for a desktop app context.
- Tauri allows us to create a pure Rust native OS webview, without the overhead of your average Electron app. This brings the bundle size and average memory usage down dramatically. It also contributes to a more native feel, especially on macOS due to Safari's close integration with the OS.
- The core (`sdcore`) is written in pure Rust.
- Typesafe communication for an RPC-like message passing system between Rust and React Query. 

## Monorepo structure:

### Apps:

- `desktop`: A [Tauri](https://tauri.studio) app with embedded `sdcore` Rust binary.
- `mobile`: A [React Native](https://reactnative.dev/) app with embedded `sdcore` Rust binary.
- `web`: A [React](https://reactjs.org) webapp as a light wrapper around the `interface` with a websocket Transport.
- `landing`: A [React](https://reactjs.org) app using Vite pages, Tailwind Typography

### Core:

- `core`: The [Rust](#) core, referred to internally as `sdcore`. Contains filesystem, database and networking logic. Can be deployed in a variety of host applications.

### Packages:

- `client`: A [TypeScript](#) client library to handle dataflow via RPC between UI and the Rust core.
- `ui`: A [React](<[#](https://reactjs.org)>) Shared component library.
- `interface`: The complete user interface in React (used by apps `desktop`, `web` and `landing`)
- `config`: `eslint` configurations (includes `eslint-config-next`, `eslint-config-prettier` and all `tsconfig.json` configs used throughout the monorepo.
- `macos`: A [Swift](#) Native binary for MacOS system extensions.
- `ios`: A [Swift](#) Native binary (planned).
- `windows`: A [C#](#) Native binary (planned).
- `android`: A [Kotlin](#) Native binary (planned).
