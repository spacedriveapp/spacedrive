<p align="center">
  <p align="center">
   <img width="150" height="150" src="packages/assets/images/AppLogo.png" alt="Logo">
  </p>
	<h1 align="center"><b>Spacedrive</b></h1>
	<p align="center">
		The comeback. A file explorer from the future.
    <br />
    <a href="https://spacedrive.com"><strong>spacedrive.com »</strong></a>
    <br />
    <br />
    🚀 <strong>Development resuming with revolutionary new architecture</strong> 🚀
    <br />
    <br />
    <b>Status:</b> Core rewrite in progress ·
    <b>Stage:</b> Foundation ·
    <b>Goal:</b> Ship working VDFS in 2025
  </p>
</p>

Spacedrive is back. After learning from 500,000 installs and 34,000 stars, we're building the file manager that should have shipped: **your files, everywhere, unified**.

What started as an ambitious vision became an engineering lesson. Now we're shipping that vision with battle-tested architecture.

<br/>

> **The Revolution**
>
> Copy files between your iPhone and MacBook as easily as moving between folders. Search across all your devices with a single query. Organize photos that live anywhere. **Device boundaries disappear.**

<p align="center">
  <img src="apps/landing/public/github.webp" alt="App screenshot">
  <br />
  <br />
  <a href="https://discord.gg/gTaF2Z44f5">
    <img src="https://img.shields.io/discord/949090953497567312?label=Discord&color=5865F2" />
  </a>
  <a href="https://x.com/spacedriveapp">
    <img src="https://img.shields.io/badge/Twitter-black?logo=x&logoColor=white" />
  </a>
  <a href="https://instagram.com/spacedriveapp">
    <img src="https://img.shields.io/badge/Instagram-E4405F?logo=instagram&logoColor=white" />
  </a>
  <a href="https://www.gnu.org/licenses/agpl-3.0">
    <img src="https://img.shields.io/static/v1?label=Licence&message=AGPL%20v3&color=000" />
  </a>
  <img src="https://img.shields.io/static/v1?label=Stage&message=Reborn&color=2BB4AB" />
  <br />
</p>

## The Vision Realized

**Copy iPhone video to MacBook storage?** Done.
**Search across all devices instantly?** Built-in.
**Organize files that live everywhere?** Native.
**Keep it private and lightning fast?** Always.

The original Spacedrive captured imaginations with a bold promise: the **Virtual Distributed File System**. Manage all your files across all your devices as if they were one giant drive. We delivered impressive file management, but the revolutionary cross-device magic remained just out of reach.

**Now it's real.**

## What Makes This Different

Your files are scattered across devices, cloud services, and external drives. Traditional file managers trap you in local boundaries. Spacedrive makes those boundaries disappear:

**🌐 Universal File Access**

- Browse files on any device from any device
- External drives, cloud storage, remote servers - all unified
- Offline files show up with cached metadata

**⚡ Lightning Search**

- Find files across all locations with a single search
- Content search inside documents, PDFs, and media
- AI-powered semantic search: "find sunset photos from vacation"

**🔄 Seamless Operations**

- Copy, move, and organize files between any devices
- Drag and drop across device boundaries
- Batch operations on distributed collections

**🔒 Privacy First**

- Your data stays on your devices
- Optional cloud sync, never required
- End-to-end encryption for all transfers

## The Journey: Lessons Learned

The original Spacedrive got 500,000 installs because the vision was right. Development paused because the execution was flawed:

### The Problems (2022-2024)

- **Split personality**: Couldn't copy between different location types
- **Search limitations**: Basic filename matching, not true content discovery
- **Technical debt**: Built on foundations that couldn't scale
- **Feature paralysis**: Perfect became the enemy of good

### The Breakthrough (2024-2025)

- **Unified experience**: Every operation works everywhere
- **Real search**: Content indexing, semantic understanding, instant results
- **Modern foundation**: Built for performance and extensibility
- **Ship early, improve fast**: Real features over perfect architecture

We kept the revolutionary vision. We rebuilt the foundation to deliver it.

## Experience the New Spacedrive

### Desktop App: Your Command Center

```
┌─ Spacedrive ──────────────────────────────────────────┐
│ ≡ Locations           📱 iPhone (via P2P)             │
│   📁 Desktop            📁 Photos (1,234 items)       │
│   📁 Documents          📁 Documents                   │
│   📁 Downloads          🔗 iCloud Drive               │
│   💾 External Drive     📱 iPad                       │
│   ☁️  iCloud Drive       📱 Android Phone             │
│   🖥️  Server             ⚙️  Background indexing...   │
└───────────────────────────────────────────────────────┘
```

**Cross-device operations made simple:**

- Drag photos from your iPhone to external storage
- Search finds files regardless of which device they're on
- Organize distributed media collections as if they were local

### CLI & Server: Power User Paradise

```bash
# Start the core daemon
spacedrive start

# Manage your digital life from anywhere
spacedrive search "presentation slides" --device laptop
spacedrive copy iPhone:/DCIM/vacation.mov ~/Desktop/
spacedrive sync-status --all-devices

# Server mode: access from anywhere
spacedrive server --host 0.0.0.0 --port 8080
```

**Perfect for:**

- **Creators**: Manage media across multiple workstations
- **Developers**: Sync projects between dev environments
- **Families**: Shared photo organization across devices
- **Self-hosters**: Private cloud with true file management

### Web Interface: Universal Access

Access your files from any browser, anywhere. Full Spacedrive functionality without installing anything.

## Architecture: Built to Last

### Self-Contained Libraries

```
My Photos.sdlibrary/
├── library.json      # Configuration & device registry
├── database.db       # All metadata and search indices
├── thumbnails/       # Generated previews
└── .lock            # Concurrency protection
```

**Portable by design:**

- **Backup** = copy the folder
- **Share** = send the folder
- **Migrate** = move the folder

### Unified Operations

No more confusion between "indexed" and "direct" files. Every file operation works the same way:

- **Indexed locations**: Rich metadata, lightning search, smart organization
- **Direct access**: Immediate operations, no waiting for scans
- **Hybrid mode**: Best of both worlds automatically

### Real Search Engine

```
🔍 Search: "sunset photos from vacation"

Results across all devices:
📱 iPhone/Photos/Vacation2024/sunset_beach.jpg
💾 External/Backup/2024/vacation_sunset.mov
☁️  iCloud/Memories/golden_hour_sunset.heic
```

**Beyond filename matching:**

- Full-text content search in documents
- Image recognition and scene detection
- Vector search for semantic queries
- Instant results even for offline files

## What's Shipping: The VDFS Roadmap

### Q1 2025: Foundation

- ✅ **Core rewrite** with unified file system
- ✅ **Working CLI** with daemon architecture
- 🚧 **Desktop app** rebuilt on new foundation
- 🚧 **Real search** with content indexing

### Q2 2025: Device Communication

- 🔄 **P2P discovery** and secure connections
- 🔄 **Cross-device operations** (copy, move, sync)
- 🔄 **Mobile apps** with desktop feature parity
- 🔄 **Web interface** for universal access

### Q3 2025: Intelligence

- 🎯 **AI-powered organization** with local models
- 🎯 **Smart collections** and auto-tagging
- 🎯 **Cloud integrations** (iCloud, Google Drive, etc.)
- 🎯 **Advanced media analysis**

### Q4 2025: Ecosystem

- 🚀 **Extension system** for community features
- 🚀 **Professional tools** for creators and teams
- 🚀 **Enterprise features** and compliance
- 🚀 **Plugin marketplace** and developer APIs

## Try It Today

The foundation is working. The CLI proves the architecture:

```bash
# Download and install
curl -fsSL https://install.spacedrive.com | sh

# Create your first library
spacedrive library create "My Files"

# Add locations across devices
spacedrive location add ~/Documents
spacedrive location add /media/external

# Smart indexing
spacedrive index ~/Pictures --deep    # Full analysis with AI
spacedrive browse /tmp --ephemeral    # Quick exploration

# Real-time monitoring
spacedrive job monitor
```

**Working today:**

- ✅ Multi-location management
- ✅ Smart indexing with progress tracking
- ✅ Content-aware search
- ✅ Real-time job monitoring
- ✅ Portable library format

## Sustainable Open Source

### Always Free & Open

- **Core file management** and VDFS operations
- **Local search** and organization features
- **P2P sync** between your own devices
- **Privacy-first** architecture

### Premium Value-Adds

- **Spacedrive Cloud**: Cross-internet sync and backup
- **Advanced AI**: Professional media analysis and organization
- **Team features**: Shared libraries and collaboration
- **Enterprise**: SSO, compliance, and enterprise deployment

### Community First

- **Weekly dev streams** showing real progress
- **Open roadmap** with community voting
- **Contributor rewards** and recognition program
- **Plugin marketplace** revenue sharing

## Why It Will Work This Time

### Technical Maturity

From 500k installs and 34k stars, we learned what users actually need:

- **Performance first**: Sub-second search, responsive UI, efficient sync
- **Reliability**: Robust error handling, data integrity, graceful failures
- **Simplicity**: Complex features with simple interfaces

### Market Reality

The world has changed since 2022:

- **Privacy concerns** have intensified with cloud services
- **AI expectations** for semantic search and smart organization
- **Multi-device life** is now universal, not niche
- **Creator economy** needs professional file management tools

### Execution Discipline

No more feature paralysis:

- **Ship working features**, enhance over time
- **Measure real usage**, not just code metrics
- **Community feedback** drives priority decisions
- **Multiple revenue streams** support sustainable development

## Get Involved

### For Users

- ⭐ **Star the repo** to follow development
- 💬 **Join Discord** for updates and early access
- 🐛 **Report issues** and request features
- 📖 **Beta testing** as features ship

### For Developers

- 🔧 **Contribute code** to the core rewrite
- 📚 **Improve docs** and tutorials
- 🧪 **Write tests** and benchmarks
- 🎨 **Design interfaces** for new features

### For Organizations

- 💼 **Early access** to enterprise features
- 🤝 **Partnership** opportunities
- 💰 **Sponsorship** and development funding
- 🎯 **Custom development** services

## The Return

Spacedrive paused because we built complexity where we needed simplicity. We solved perfect problems instead of real problems. We got paralyzed by architectural purity instead of shipping user value.

**Now we're back with wisdom.**

The vision was right: files scattered across devices need a unified experience. The execution was wrong: we over-engineered where we should have shipped.

The future of file management isn't about better folder hierarchies or cloud storage. It's about making all your files feel local, searchable, and organized - regardless of where they actually live.

**That future is shipping in 2025.**

---

<p align="center">
  <strong>Follow the comeback</strong><br/>
  <a href="https://spacedrive.com">Website</a> ·
  <a href="https://discord.gg/gTaF2Z44f5">Discord</a> ·
  <a href="https://x.com/spacedriveapp">Twitter</a> ·
  <a href="https://github.com/spacedriveapp/spacedrive/tree/main/core">Core Development</a>
</p>

<p align="center">
  <em>The file manager that should exist. Finally being built right.</em>
</p>
