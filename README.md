<p align="center">
  <img width="150" height="150" src="packages/assets/images/AppLogo.png" alt="Spacedrive Logo">
  <h1 align="center">Spacedrive</h1>
  <p align="center">
    A Virtual Distributed File System (VDFS) for all your data.
    <br />
    <a href="https://spacedrive.com"><strong>spacedrive.com</strong></a>
    ·
    <a href="https://discord.gg/gTaF2Z44f5"><strong>Discord</strong></a>
    ·
    <a href="https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md"><strong>Read the Whitepaper</strong></a>
  </p>
  <p align="center">
    <a href="https://discord.gg/gTaF2Z44f5">
      <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
    </a>
    <a href="https://www.gnu.org/licenses/agpl-3.0">
      <img src="https://img.shields.io/static/v1?label=Licence&message=AGPL%20v3&color=000" />
    </a>
    <img src="https://img.shields.io/static/v1?label=Stage&message=Foundation&color=2BB4AB" />
    <img src="https://img.shields.io/static/v1?label=Core&message=Rust&color=DEA584" />
  </p>
</p>

Your files are scattered across devices, cloud accounts, and external drives. Spacedrive unifies them into a single, virtual library, no matter where they are. It's a file manager from the future, built on a **Virtual Distributed File System (VDFS)**.

Browse, search, and organize everything from one place. Device boundaries disappear.

---

## The Vision: Your Data, Unified

Spacedrive is not another cloud storage service. It's a virtual layer that connects your existing storage locations. Your files stay on your devices, under your control.

- **Copy a file from your phone to your PC** as if they were two folders on the same computer.
- **Search for a document** and find it, whether it's on your laptop, a connected drive, or a remote server.
- **Organize photo libraries** that span multiple devices into a single, beautiful collection.

This is made possible by a new architecture, rebuilt from the ground up based on the lessons learned from over 500,000 installs of our first version.

## How It Works: Core Concepts

Spacedrive V2 is built on a set of foundational pillars designed for performance, privacy, and scalability. The entire core is implemented in **Rust** on a modern, asynchronous stack.

#### 1. The Virtual Distributed File System (VDFS)

The VDFS is the heart of Spacedrive. It creates a unified namespace for all your data using a universal addressing system called `SdPath`. It doesn't matter if a file is on `C:\Users\...` or `~/Documents` or a remote server—Spacedrive gives it a stable, virtual address.
<br/>_➤ Learn more in the [Whitepaper (Section 4.1)](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)._

#### 2. Content-Addressable Storage

Spacedrive uses a **Content Identity System** to understand your data at the byte level. This provides intelligent, cross-device deduplication and powers a "Data Guardian" feature that monitors data redundancy to protect against loss.
<br/>_➤ Learn more in the [Whitepaper (Section 4.4)](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)._

#### 3. Leaderless, Offline-First Sync

Spacedrive uses a novel, **leaderless synchronization model** that combines state-based and log-based replication. This eliminates the need for a central coordinator, making sync faster, more resilient, and fully functional offline. Changes are efficiently transferred using the **Iroh** P2P networking library.
<br/>_➤ Learn more in the [Whitepaper (Section 4.5)](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)._

#### 4. Transactional, Verifiable Actions

All file operations (copy, move, delete) are treated as **transactions**. You can preview the outcome of an operation before it executes, preventing errors and data loss. The system guarantees that operations will eventually complete, even across offline devices.
<br/>_➤ Learn more in the [Whitepaper (Section 4.2)](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)._

## Project Status: Foundation Shipped

The original vision for Spacedrive was ambitious. The first version captured imaginations but hit architectural limits. We paused, redesigned the system from first principles, and have now completed the foundational rewrite.

- **✅ Core Engine:** The VDFS core, written in Rust, is complete.
- **✅ CLI:** A powerful command-line interface is available for use today.
- **⏳ Desktop & Mobile Apps:** GUI applications are in active development, built on the new core.

We are building in the open. You can follow our progress through our [open tasks](.tasks/) and on Discord.

## Get Started with the CLI

The foundation is working. The CLI proves the architecture.

```bash
# Download and install
curl -fsSL https://install.spacedrive.com | sh

# Create your first library
sd library create "My Library"

# Add a local directory to your library
sd location add ~/Pictures

# See the contents of your virtual filesystem
sd ls
```

## Why It Will Work This Time

- **Technical Maturity:** The new architecture is designed to solve the hard problems of distributed systems from the start. We are not retrofitting; we are building on a solid foundation.
- **Execution Discipline:** We are focused on shipping a stable core first, then expanding to more platforms and features. No more feature paralysis.
- **Community Transparency:** Our whitepaper, design documents, and roadmap are public. We invite you to review our work and contribute.

## Get Involved

- **⭐ Star the repo** to follow along.
- **💬 Join the [Discord](https://discord.gg/gTaF2Z44f5)** to chat with the developers and community.
- **📖 Read the [Whitepaper](https://github.com/spacedriveapp/spacedrive/blob/main/docs/whitepaper.md)** to understand the full vision.
- **💻 Contribute:** Check out the [contribution guide](CONTRIBUTING.md) and open tasks.

---

<p align="center">
  <em>The file manager that should exist. Finally being built right.</em>
</p>
