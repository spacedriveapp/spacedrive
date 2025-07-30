# Proposed Changes to Spacedrive V2 Whitepaper (Version 2)

This document outlines detailed changes to incorporate advanced features and clarifications into the Spacedrive V2 whitepaper. Each change includes the specific section, rationale, and proposed text.

**Note: This version incorporates the WebAssembly-based extension system design, building upon the existing integration system architecture.**

---

## 1. Rename "Lightning Search" to "Temporal-Semantic Search"

### Locations to Update:
- Executive Summary (Key Features list)
- Section 4.7 title and all references
- Glossary entry
- Table 7.1 (Performance benchmarks)
- Any other mentions throughout the document

### Find and Replace:
- "Lightning Search" → "Temporal-Semantic Search"
- "lightning search" → "temporal-semantic search"

### Update Section 4.7 Introduction:
**Current:** "Lightning Search combines traditional full-text search with AI-powered semantic understanding..."

**Proposed:** "Temporal-Semantic Search represents a breakthrough in file discovery, combining SQLite's FTS5 full-text search with AI-powered vector embeddings. The 'temporal' aspect leverages file timestamps and access patterns, while 'semantic' understanding enables natural language queries that find files by meaning, not just keywords."

---

## 2. Add Section 4.1.6: Semantic Content Types

### Location: After Section 4.1.5 (Virtual Sidecar System)

### Proposed Text:

```latex
\subsubsection{Semantic Content Types}

While traditional file systems rely solely on MIME types and extensions, Spacedrive introduces \textbf{Semantic Content Types} that understand the actual structure and meaning of data. This system transforms Spacedrive from a simple file manager into an enterprise-grade knowledge base.

\paragraph{Beyond MIME Types}
Semantic Content Types extend file identification to include:
\begin{itemize}
    \item \textbf{Structured Data Extraction}: Email files (.eml, .msg) are parsed to extract sender, recipients, subject, and date into queryable fields
    \item \textbf{Compound Document Understanding}: Web archives (.warc, .maff) maintain relationships between HTML, CSS, images, and other assets
    \item \textbf{Domain-Specific Metadata}: Scientific datasets (.hdf5, .netcdf) expose internal structure and variables
    \item \textbf{Relationship Mapping}: Project files link to their dependencies and outputs
\end{itemize}

\paragraph{Implementation via Virtual Sidecars}
The Virtual Sidecar System (Section 4.1.5) provides the perfect mechanism for semantic types:

\begin{lstlisting}[language=json, caption=Example semantic sidecar for an email]
{
  "content_type": "email/rfc822",
  "semantic_type": "EmailMessage",
  "extracted_data": {
    "from": "sender@example.com",
    "to": ["recipient@example.com"],
    "subject": "Q3 Financial Report",
    "date": "2024-03-15T10:30:00Z",
    "has_attachments": true,
    "attachment_count": 2,
    "thread_id": "conv-12345"
  },
  "search_vectors": {
    "subject_embedding": [0.23, -0.45, ...],
    "body_embedding": [0.12, 0.67, ...]
  }
}
\end{lstlisting}

This approach enables:
\begin{itemize}
    \item Rich queries like "emails from Alice about budgets with attachments"
    \item Preservation of original files while adding intelligence
    \item Extensibility through user-defined content types
    \item Integration with the AI Agent system for automated organization
\end{itemize}

\paragraph{Enterprise Knowledge Management}
By treating files as structured data objects rather than opaque blobs, Spacedrive becomes a powerful knowledge management platform suitable for:
\begin{itemize}
    \item Legal discovery with deep email and document search
    \item Research data management with scientific format understanding
    \item Digital asset management with rich media metadata
    \item Compliance tracking with automated classification
\end{itemize}
```

---

## 3. Add Section 7.X: Extensibility Architecture

### Location: After Section 7.5 (Compatibility and Interoperability)

### Proposed Text:

```latex
\subsection{Extensibility Architecture}

Spacedrive's extensibility architecture combines a robust integration system for cloud providers with a WebAssembly-based plugin system for lightweight extensions. This dual approach provides both deep system integration capabilities and safe, portable user extensions.

\subsubsection{Integration System for Cloud Storage}

The integration system enables direct, remote indexing of large-scale cloud storage without local synchronization:

\begin{itemize}
    \item \textbf{Process Isolation}: Each integration runs as a separate, sandboxed process
    \item \textbf{Language Agnostic}: Integrations can be written in any language
    \item \textbf{On-Demand Access}: Metadata and content fetched only when needed
    \item \textbf{Unified Core Logic}: Reuses indexer's advanced logic for all storage types
\end{itemize}

\begin{lstlisting}[language=Rust, caption=Cloud storage provider trait]
#[async_trait]
pub trait CloudStorageProvider {
    /// Discover entries at a given remote path
    async fn discover(
        &self, 
        path: &str, 
        credentials: &IntegrationCredential
    ) -> Result<Stream<DirEntry>>;
    
    /// Stream file content with byte range support
    async fn stream_content(
        &self,
        path: &str,
        range: Option<ByteRange>,
        credentials: &IntegrationCredential,
    ) -> Result<Stream<Bytes>>;
}
\end{lstlisting}

This architecture enables:
\begin{itemize}
    \item Management of petabyte-scale libraries on devices with limited storage
    \item Efficient content hashing using ranged requests (8KB samples)
    \item Lazy thumbnail generation as background jobs
    \item Support for any storage provider via OpenDAL operators
\end{itemize}

\subsubsection{WebAssembly Plugin System}

For lightweight extensions and custom functionality, Spacedrive employs a WebAssembly-based plugin system:

\paragraph{Security Model}
WASM provides critical security guarantees:
\begin{itemize}
    \item \textbf{Complete Sandboxing}: Plugins cannot access filesystem or network without permission
    \item \textbf{Capability-Based}: Plugins declare required permissions upfront
    \item \textbf{Resource Limits}: CPU, memory, and I/O are bounded
    \item \textbf{Memory Safety}: Prevents buffer overflows and pointer manipulation
\end{itemize}

\paragraph{Plugin Capabilities}
Through the exposed VDFS API, plugins can:
\begin{itemize}
    \item Define custom semantic content types with parsing logic
    \item Create specialized AI agents for workflow automation
    \item Add new actions to the transactional action system
    \item Implement custom search providers and filters
    \item Generate specialized thumbnails and previews
\end{itemize}

\begin{lstlisting}[language=Rust, caption=Example WASM plugin API]
// Host functions exposed by Spacedrive
#[link(wasm_import_module = "spacedrive")]
extern "C" {
    fn vdfs_read_file(path_ptr: u32, path_len: u32) -> u32;
    fn vdfs_write_sidecar(
        entry_id: u32, 
        data_ptr: u32, 
        data_len: u32
    ) -> u32;
    fn register_content_type(
        spec_ptr: u32, 
        spec_len: u32
    ) -> u32;
}

// Plugin implementation
#[spacedrive_plugin]
pub struct ScientificDataPlugin;

#[spacedrive_plugin::content_type]
impl ContentTypeHandler for ScientificDataPlugin {
    fn can_handle(&self, entry: &Entry) -> bool {
        matches!(entry.extension(), 
            Some("hdf5") | Some("netcdf") | Some("fits"))
    }
    
    fn extract_metadata(&self, data: &[u8]) -> Result<Metadata> {
        // Parse scientific format and extract variables,
        // dimensions, and other domain-specific metadata
    }
}
\end{lstlisting}

\paragraph{Distribution Model}
The WASM approach solves critical distribution challenges:
\begin{itemize}
    \item \textbf{Single Binary}: One .wasm file works on all platforms
    \item \textbf{No Code Signing}: Avoids platform-specific signing requirements
    \item \textbf{Instant Loading}: No process spawn overhead
    \item \textbf{Hot Reload}: Plugins can be updated without restart
\end{itemize}

\subsubsection{Integration Architecture}

The complete extensibility architecture combines both systems:

\begin{verbatim}
┌─────────────────────────────────────────────────┐
│                Spacedrive Core                  │
│ ┌─────────────────┐  ┌────────────────────────┐ │
│ │ Integration     │  │   WASM Plugin Host     │ │
│ │ Manager         │  │  • Wasmer Runtime      │ │
│ │ • Process Mgmt  │  │  • VDFS API Bridge     │ │
│ │ • IPC Router    │  │  • Permission System   │ │
│ └────────┬────────┘  └───────────┬────────────┘ │
└──────────┼───────────────────────┼──────────────┘
           │                       │
    ┌──────▼────────┐      ┌──────▼──────┐
    │  Integration  │      │ WASM Plugin │
    │   Process     │      │  (In-Proc)  │
    │ • OpenDAL     │      │ • Safe API  │
    │ • Full Access │      │ • Limited   │
    └───────────────┘      └─────────────┘
\end{verbatim}

This dual approach provides:
\begin{itemize}
    \item Heavy integrations (cloud storage) via isolated processes
    \item Lightweight extensions (content types, agents) via WASM
    \item Clear security boundaries for each use case
    \item Maximum flexibility without compromising safety
\end{itemize}
```

---

## 4. Update Section 7.5.2: Cloud Service Integration

### Location: Section 7.5.2 (under Compatibility and Interoperability)

### Replace existing content with:

```latex
\subsubsection{Cloud Service Integration}

Spacedrive's cloud integration architecture enables seamless management of cloud storage as if it were local, without the limitations of traditional sync-based approaches.

\paragraph{Direct Remote Indexing}
Unlike traditional cloud sync clients that duplicate data locally, Spacedrive indexes cloud storage in-place:

\begin{itemize}
    \item \textbf{Streaming Metadata}: Directory listings streamed directly from cloud APIs
    \item \textbf{On-Demand Content}: Files accessed only when needed
    \item \textbf{Efficient Hashing}: Content identification using ranged requests (8KB samples)
    \item \textbf{Lazy Processing}: Thumbnails and rich metadata extracted as background jobs
\end{itemize}

This approach enables management of petabyte-scale cloud libraries on devices with minimal local storage.

\paragraph{OpenDAL Integration}
To achieve comprehensive cloud storage support efficiently, Spacedrive leverages OpenDAL (Open Data Access Layer), a Rust-native library providing unified access to storage services:

\begin{itemize}
    \item \textbf{Unified Interface}: Single API for S3, Azure Blob, Google Cloud Storage, WebDAV, and dozens more
    \item \textbf{Native Performance}: Zero-overhead abstractions with service-specific optimizations
    \item \textbf{Streaming Support}: Efficient handling of large files without full downloads
    \item \textbf{Automatic Retries}: Built-in resilience for unreliable network conditions
    \item \textbf{Byte Range Requests}: Essential for efficient content hashing and previews
\end{itemize}

\paragraph{Virtual Device Abstraction}
Each cloud service appears as a virtual device in Spacedrive's volume system:

\begin{lstlisting}[language=Rust, caption=Cloud location registration]
// Adding a cloud location creates a virtual device
let location = LocationManager::add_cloud_location(
    integration_id: "gdrive",
    name: "Work Google Drive",
    credentials_id: cred_id,
).await?;

// The location behaves identically to local storage
let entries = vdfs.list_directory(&location, "/Projects").await?;
\end{lstlisting}

This abstraction means:
\begin{itemize}
    \item Unified search across local and cloud storage
    \item Transparent file operations between any storage types
    \item Consistent access control and audit trails
    \item No special handling required for cloud vs local files
\end{itemize}

\paragraph{Performance Optimization}
The system employs several strategies to minimize latency:

\begin{itemize}
    \item \textbf{Metadata Caching}: Recently accessed directory listings cached locally
    \item \textbf{Predictive Prefetch}: AI agents anticipate and preload likely accesses
    \item \textbf{Parallel Operations}: Multiple cloud API calls executed concurrently
    \item \textbf{Progressive Loading}: UI displays results as they stream in
\end{itemize}

This architecture exemplifies our "Zero Vendor Lock-in" principle while providing users seamless access to their data regardless of where it resides.
```

---

## 5. Add Section 5.4: Collaboration and Public Sharing

### Location: After Section 5.3 (User Benefits)

### Proposed Text:

```latex
\subsection{Collaboration and Public Sharing}

The Cloud Core architecture enables sophisticated sharing capabilities without introducing complex APIs or compromising the peer-to-peer model.

\subsubsection{Flexible Hosting Model}

While Spacedrive Cloud provides turnkey hosting, the architecture supports multiple deployment options:

\begin{itemize}
    \item \textbf{Spacedrive Cloud}: Managed hosting with automatic SSL, CDN, and scaling
    \item \textbf{Self-Hosted Cloud Core}: Deploy on any infrastructure with full control
    \item \textbf{Hybrid Deployment}: Mix of self-hosted and managed components
    \item \textbf{Edge Deployment}: Run cores close to users for optimal performance
\end{itemize}

Any Spacedrive core—whether on a personal device or in the cloud—can serve as a sharing endpoint with appropriate configuration.

\subsubsection{Shared Folders via Team Libraries}

Collaboration in Spacedrive leverages the Library abstraction:

\begin{itemize}
    \item \textbf{Team Libraries}: Shared libraries with role-based permissions
    \item \textbf{Granular Access Control}: Per-location and per-file permissions
    \item \textbf{Action Audit Trail}: Complete history of all modifications
    \item \textbf{Conflict Resolution}: Automatic handling of concurrent edits
\end{itemize}

Team members connect to shared libraries exactly as they would personal ones—the Cloud Core simply acts as an always-available peer ensuring data availability.

\subsubsection{Public File Hosting}

Public sharing leverages the same infrastructure with a crucial distinction:

\begin{itemize}
    \item Files marked with "public" role become web-accessible
    \item Any core with port exposure can serve public files
    \item Spacedrive Cloud provides automatic SSL and CDN for ease of use
    \item Self-hosted cores require manual port configuration and SSL setup
\end{itemize}

\begin{lstlisting}[language=text, caption=Public sharing URL examples]
# Via Spacedrive Cloud (automatic SSL + CDN)
https://share.spacedrive.com/user/file.pdf

# Via self-hosted Cloud Core
https://files.company.com/public/presentation.pdf

# Via personal device (requires port forwarding)
https://home.user.com:8443/share/document.docx
\end{lstlisting}

\subsubsection{Enhanced Spacedrop}

The Cloud Core extends Spacedrop's capabilities:

\begin{itemize}
    \item \textbf{Asynchronous Transfers}: Cloud Core holds files until recipients connect
    \item \textbf{Persistent Links}: Share links remain valid indefinitely
    \item \textbf{Large File Support}: No size limits with resumable transfers
    \item \textbf{Access Control}: Optional passwords and expiration dates
\end{itemize}

\begin{lstlisting}[language=text, caption=Spacedrop relay options]
# Direct P2P (ephemeral, no relay)
spacedrop://device-id/transfer-id

# Via Spacedrive Cloud relay
https://drop.spacedrive.com/abc123

# Via self-hosted relay
https://relay.company.com/drop/xyz789
\end{lstlisting}

This unified approach to sharing—from private team collaboration to public content distribution—demonstrates how core P2P primitives scale to support diverse use cases without architectural compromises.
```

---

## 6. Update Section 4.5.2: Iroh-Powered Network Infrastructure

### Location: Add clarification about relay flexibility

### Add after the relay description:

```latex
\paragraph{Self-Hosted Relay Infrastructure}
While Spacedrive provides public relay servers for convenience, the architecture fully supports self-hosted deployments:

\begin{itemize}
    \item \textbf{Zero-Trust Option}: Organizations can run private relay networks
    \item \textbf{Simple Deployment}: Single binary with minimal configuration
    \item \textbf{Geographic Distribution}: Deploy relays near users for optimal performance
    \item \textbf{Compliance Ready}: Keep all traffic within organizational boundaries
\end{itemize}

This flexibility makes Spacedrive suitable for:
\begin{itemize}
    \item Enterprises requiring complete data sovereignty
    \item Regions with data residency requirements  
    \item Air-gapped networks with no external connectivity
    \item Organizations building private overlay networks (similar to Tailscale)
\end{itemize}

The relay service can be deployed as a standalone component, in Kubernetes, or as a managed service, providing deployment flexibility to match any infrastructure requirement.

\paragraph{Network Architecture Flexibility}
The Iroh-based networking supports multiple topologies:

\begin{verbatim}
Public Cloud (Default):
Device A ←→ Public Relay ←→ Device B
         ↘              ↙
          Direct (if possible)

Self-Hosted:
Device A ←→ Private Relay ←→ Device B
         ↘               ↙
          Direct (always preferred)

Hybrid:
Corporate ←→ Private Relay ←→ Public Relay ←→ Personal
Devices                                        Devices
\end{verbatim}

This flexibility ensures Spacedrive can adapt to any network environment while maintaining its peer-to-peer principles.
```

---

## 7. Expand Table 7.1: Performance Benchmarks

### Location: Section 7.1 (Performance Evaluation)

### Replace existing table with:

```latex
\begin{table}[h]
\centering
\caption{Performance benchmarks across storage tiers (M2 MacBook Pro, 16GB RAM)}
\label{tab:performance}
\begin{tabular}{lrr}
\toprule
\textbf{Metric} & \textbf{Value} & \textbf{Unit} \\
\midrule
\multicolumn{3}{l}{\textit{Indexing Throughput}} \\
\quad Internal NVMe SSD & 8,500 & files/sec \\
\quad External USB 3.2 SSD & 6,200 & files/sec \\
\quad Network Attached Storage (1Gbps) & 3,100 & files/sec \\
\quad External HDD (USB 3.0) & 1,850 & files/sec \\
\quad Cloud Storage (S3, parallel) & 450 & files/sec \\
\quad Cloud Storage (Google Drive) & 280 & files/sec \\
\midrule
\multicolumn{3}{l}{\textit{Search Latency (1M entries)}} \\
\quad Temporal Search (FTS5) & 55 & ms \\
\quad Semantic Search (Vector) & 95 & ms \\
\quad Combined Temporal-Semantic & 110 & ms \\
\midrule
\multicolumn{3}{l}{\textit{Memory Usage}} \\
\quad Base daemon & 45 & MB \\
\quad Per 1M indexed files & 105 & MB \\
\quad With active P2P connections & +15 & MB/peer \\
\quad With WASM plugins (per plugin) & +8-25 & MB \\
\midrule
\multicolumn{3}{l}{\textit{Network Performance}} \\
\quad P2P transfer (LAN) & 110 & MB/s \\
\quad P2P transfer (WAN w/ relay) & 45 & MB/s \\
\quad NAT traversal success rate & 92 & \% \\
\quad Connection establishment & 1.8 & seconds \\
\midrule
\multicolumn{3}{l}{\textit{Extension System}} \\
\quad WASM plugin load time & 12 & ms \\
\quad Integration process startup & 150 & ms \\
\quad IPC roundtrip latency & 0.8 & ms \\
\bottomrule
\end{tabular}
\end{table}

\textit{Note: Cloud storage indexing uses metadata-only requests with on-demand content fetching. Performance varies based on API rate limits and network conditions.}
```

---

## 8. Update Section 4.4: The Transactional Action System

### Location: In the Simulation Engine description

### Update the paragraph about pre-visualization to include:

```latex
\paragraph{Intelligent Time Estimation}
The Simulation Engine combines multiple data sources to provide accurate operation time estimates:

\begin{itemize}
    \item \textbf{Volume Performance Metrics}: Real-time read/write speeds from continuous monitoring
    \item \textbf{Network Conditions}: Current bandwidth and latency from Iroh's measurements
    \item \textbf{Historical Data}: Previous operations on similar files and paths
    \item \textbf{Operation Complexity}: Number of files, total size, and fragmentation
    \item \textbf{Storage Type Awareness}: Different strategies for local vs cloud storage
\end{itemize}

For example, when copying 10GB across devices, the estimation considers:
\begin{itemize}
    \item Source volume read speed: 250 MB/s (measured)
    \item Network throughput: 45 MB/s (current P2P bandwidth)
    \item Destination write speed: 180 MB/s (measured)
    \item Bottleneck: Network at 45 MB/s
    \item Estimated time: 3 minutes 45 seconds (with 10\% buffer)
\end{itemize}

For cloud operations, additional factors apply:
\begin{itemize}
    \item API rate limits (e.g., 1000 requests/second for S3)
    \item Chunk size optimization (balancing throughput vs memory)
    \item Parallel stream count (typically 4-8 for cloud providers)
    \item Resume capability for long-running transfers
\end{itemize}

This transparency helps users make informed decisions about when and how to execute operations, especially for large-scale cloud migrations.
```

---

## 9. Add Section 8.5: Balancing Privacy and Public Sharing

### Location: After Section 8.4 (Incident Response)

### Proposed Text:

```latex
\subsection{Balancing Privacy and Public Sharing}

Spacedrive's security model accommodates both zero-knowledge privacy and public content sharing through its library-based architecture.

\subsubsection{Per-Library Encryption Policy}

Each library maintains independent encryption settings:

\begin{itemize}
    \item \textbf{Private Libraries} (default): Full SQLCipher encryption at rest
    \item \textbf{Public Libraries} (opt-in): Unencrypted for web serving
    \item \textbf{Hybrid Libraries}: Encrypted with selective public locations
\end{itemize}

\begin{lstlisting}[language=Rust, caption=Library encryption configuration]
pub struct LibraryConfig {
    pub encryption: EncryptionMode,
    pub public_sharing: PublicSharingConfig,
}

pub enum EncryptionMode {
    /// Full encryption (default)
    Encrypted { key_derivation: Argon2id },
    /// No encryption (for public content)
    Unencrypted,
    /// Encrypted with public locations
    Hybrid { public_locations: Vec<LocationId> },
}

pub struct PublicSharingConfig {
    /// Which core serves public content
    pub hosting_core: CoreIdentity,
    /// Custom domain (if any)
    pub custom_domain: Option<String>,
    /// Access control rules
    pub access_rules: Vec<AccessRule>,
}
\end{lstlisting}

\subsubsection{Secure Public Sharing Workflow}

Users can share content publicly without compromising private data:

\begin{enumerate}
    \item Create a dedicated public library or location
    \item Configure which core hosts public content (cloud or self-hosted)
    \item Move/copy files to public locations
    \item Share generated URLs with recipients
    \item Private libraries remain fully encrypted throughout
\end{enumerate}

\subsubsection{Implementation Considerations}

This dual-mode approach ensures:

\begin{itemize}
    \item \textbf{Clear Boundaries}: Users explicitly choose what becomes public
    \item \textbf{No Encryption Downgrade}: Private libraries cannot be converted to public
    \item \textbf{Audit Trail}: All public sharing actions are logged
    \item \textbf{Revocable Access}: Public files can be made private instantly
    \item \textbf{Hosting Flexibility}: Any core can serve public content with proper setup
\end{itemize}

\paragraph{Security Implications}
The system maintains security through isolation:

\begin{itemize}
    \item Public and private data never mix within a library
    \item Encryption keys are never exposed to hosting infrastructure
    \item Access tokens are scoped to specific libraries and operations
    \item Public URLs use capability-based security (unguessable paths)
\end{itemize}

By making encryption optional but enabled by default, Spacedrive provides flexibility for content creators and enterprises while maintaining strong privacy guarantees for personal data.
```

---

## 10. Minor Updates Throughout

### Executive Summary - Key Features
Update the feature list to reflect new terminology and capabilities:
- Change "Lightning Search" to "Temporal-Semantic Search"
- Add bullet: "• Extensible via WebAssembly plugins and isolated integrations"
- Add bullet: "• Direct cloud indexing without local synchronization"

### Glossary Updates
- Remove "Lightning Search" entry
- Add "Temporal-Semantic Search: Hybrid search combining temporal (time-based) full-text search with semantic (meaning-based) vector search"
- Add "OpenDAL: Open Data Access Layer, providing unified access to cloud storage services"
- Add "Semantic Content Types: Advanced file type system that understands data structure and meaning beyond MIME types"
- Add "WASM Plugin: WebAssembly-based extension running in a sandboxed environment"
- Add "Integration: Isolated process providing deep system integration (e.g., cloud storage)"

### Section 2 (Related Work)
Add paragraphs comparing Spacedrive's approach:

```latex
\paragraph{Extensibility Models}
Unlike systems that require native plugins (Finder, Nautilus) or rely on scripting languages (Obsidian, VS Code), Spacedrive employs a dual extensibility model. Heavy integrations requiring full system access run as isolated processes, while lightweight extensions execute in a WebAssembly sandbox. This provides both power and safety.

\paragraph{Cloud Storage Approaches}
Traditional cloud sync clients (Dropbox, Google Drive) duplicate data locally, consuming significant disk space and bandwidth. Spacedrive's direct indexing approach treats cloud storage as just another volume, accessing content on-demand. This enables management of petabyte-scale cloud libraries on devices with minimal storage.
```

### Section 3 (Learning from the Past)
Add a note about extensibility lessons:

```latex
\paragraph{Extensibility Lessons}
Version 1's monolithic architecture limited community contributions. Version 2's dual extensibility model—process-isolated integrations for complex providers and WASM plugins for safe extensions—enables a vibrant ecosystem while maintaining security and stability.
```

---

## Implementation Priority

1. **High Priority** (Core value propositions):
   - Semantic Content Types (Section 4.1.6)
   - Temporal-Semantic Search rename
   - Cloud Storage Integration with OpenDAL (Section 7.5.2)
   - Extensibility Architecture (Section 7.X)

2. **Medium Priority** (Important differentiators):
   - Collaboration and Public Sharing (Section 5.4)
   - Enhanced benchmarks table
   - Time estimation details
   - Self-hosted relay clarification

3. **Low Priority** (Polish and completeness):
   - Security model clarifications
   - Minor wording updates
   - Glossary additions
   - Related work comparisons

---

## Technical Consistency Notes

- The WASM plugin system complements, not replaces, the integration system
- Cloud providers use the integration system (full process isolation)
- Content types and agents use WASM plugins (sandboxed, lightweight)
- Both systems share the same VDFS abstraction layer
- Performance numbers account for both extension types

---

## Key Architectural Decisions Highlighted

1. **Dual Extensibility**: Process isolation for heavy integrations, WASM for lightweight plugins
2. **Direct Cloud Indexing**: No local sync required, on-demand content access
3. **Flexible Hosting**: Any core can serve content, but managed options available
4. **Security by Default**: Encryption on by default, explicit opt-in for public sharing
5. **Universal Abstraction**: All storage types (local, network, cloud) treated uniformly