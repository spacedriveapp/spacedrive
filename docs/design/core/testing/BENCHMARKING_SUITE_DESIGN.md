<!--CREATED: 2025-08-08-->
### Spacedrive Benchmarking Suite — Design Document

Author: Core Team
Status: Draft (for review)
Last updated: 2025-08-08

---

### 1) Objectives

- **Primary goal**: Produce repeatable, representative performance metrics for Spacedrive that we can cite confidently (and automate regression tracking).
- **Scope**: Indexing pipeline (per-phase), search, database throughput, network transfer (P2P), and provider-backed remote indexing.
- **Non-goals**: Micro-optimizing individual syscalls; publishing vendor shootouts.

---

### 2) Definitions (unambiguous metrics)

- **Indexing Throughput (Discovery-only)**: Files/sec while listing directories and creating Entry records (no content hash, no media extraction). Includes DB writes unless explicitly disabled.
- **Indexing Throughput (Discovery + Content ID)**: Files/sec when Discovery plus Content Identification (BLAKE3 sampling/full as configured) are enabled. Media extraction disabled.
- **Indexing Throughput (Full)**: Files/sec with Discovery + Content ID + Media metadata extraction enabled.
- **Content Hash Throughput**: MB/sec and files/sec for BLAKE3 with strategy: small-files full hash; large-files sampled hash.
- **Search Latency**: p50/p90/p99 latency for keyword (FTS) and semantic/vector queries at N entries.
- **DB Ingest Rate**: Entries/sec and txn latency with production PRAGMA settings (WAL, synchronous, etc.).
- **Network Transfer Throughput**: MB/sec end-to-end for P2P (LAN/WAN) under typical configurations.
- **Cloud/Remote Indexing Throughput**: Files/sec for S3, Google Drive, FTP/SFTP/WebDAV, specifying provider limits, concurrency, and metadata-only mode.

Each metric must specify: hardware, OS, dataset recipe, cache state (cold/warm), concurrency settings, and feature flags.

---

### 3) Environments

- **Hardware profiles**

  - M2 MacBook Pro, 16GB RAM, internal NVMe (macOS 14.x)
  - Linux desktop, AMD/Intel CPU, NVMe SSD (kernel ≥5.15)
  - HDD-based system (USB 3.0 or SATA HDD)
  - NAS via 1Gbps and optionally 10Gbps

- **Remote providers**

  - S3-compatible (AWS S3 or MinIO)
  - Google Drive
  - FTP/SFTP/WebDAV (local containers when possible for reproducibility)

- **Environment capture** (auto-logged into results):
  - CPU model, cores/threads; memory; OS version; disk type(s) and interface
  - Filesystem type; mount options; network link speed
  - Spacedrive commit, build flags, Rust version

---

### 4) Datasets and Sample Data Strategy

We will not check large datasets into the repo. Instead, we define deterministic, scriptable dataset “recipes”. Two sources:

1. **Synthetic Generator (primary)**

   - Deterministic via `--seed`.
   - Parameters: directory fanout/depth, file count/buckets, size distributions (tiny/small/medium/large/huge), file type mixtures (text, binary, images, audio, video), duplicate ratios, random content vs patterned content.
   - Media fixtures: generate images/videos via lightweight generators (e.g., ffmpeg image/video synthesis) when media pipelines are enabled. Sizes and durations configurable.
   - Output example layout:
     - `benchdata/<recipe-name>/` containing multiple test `Locations` (e.g., `small/`, `mixed/`, `media/`, `large/`).

2. **Scripted Real-World Corpora (optional add-ons)**
   - Fetchers that download well-known public datasets (e.g., Linux kernel source snapshot, Gutenberg text subset, a small OpenImages sample). Not run in CI by default. All licensing respected and documented.

Benchmark Recipe (YAML) — example:

```yaml
name: mixed_nvme_default
seed: 42
locations:
  - path: benchdata/mixed
    structure:
      depth: 4
      fanout_per_dir: 12
    files:
      total: 500_000
      size_buckets:
        tiny: { range: [0, 1_024], share: 0.25 }
        small: { range: [1_024, 64_000], share: 0.35 }
        medium: { range: [64_000, 5_000_000], share: 0.30 }
        large: { range: [5_000_000, 200_000_000], share: 0.09 }
        huge: { range: [200_000_000, 2_000_000_000], share: 0.01 }
      duplicate_ratio: 0.05
      media_ratio: 0.10
      extensions: [txt, rs, jpg, png, mp4, pdf, docx, zip]
media:
  generate_thumbnails: false
  synthetic_video: { enabled: true, duration_s: 5, width: 1280, height: 720 }
```

You (James) can build and curate a set of canonical recipes for different storage types. The generator will create those datasets locally; remote datasets can be mirrored to providers (S3 bucket, NAS share) using companion scripts.

---

### 5) Benchmark Harness Architecture

- **New workspace member**: `benchmarks/` (Rust crate) providing a CLI `sd-bench` with subcommands:

  - `mkdata` — generate datasets from recipe YAML
  - `run` — execute a benchmark scenario and collect results
  - `report` — aggregate and render markdown/CSV from JSON results

- **Runner (`sd-bench run`)**

  - Scenarios: `indexing-discovery`, `indexing-content-id`, `indexing-full`, `search`, `db-ingest`, `p2p-transfer`, `remote-indexing` (s3/gdrive/ftp/sftp/webdav)
  - Options: `--recipe <file>`, `--location <path>...`, `--runs 10`, `--cold-cache on|off`, `--persist on|off`, `--concurrency N`, `--features media,semantic`, `--phases discovery,content,media`
  - Output: NDJSON and summary JSON written to `benchmarks/results/<timestamp>_<scenario>.json`
  - Captures environment metadata automatically

- **Integration with Spacedrive Core**
  - Use existing CLI/daemon where possible to avoid special code paths. Prefer programmatic invocation (library API) when we need precise phase toggles and counters.
  - Expose a stable “benchmark mode” in the indexing pipeline that:
    - Enables per-phase counters and timers (files_discovered, files_hashed, bytes_read_actual, entries_persisted, db_txn_count, etc.)
    - Emits structured events via `tracing` with a stable schema
    - Runs with deterministic concurrency (configurable worker counts)

---

### 6) Instrumentation & Data Model

- **Instrumentation points** (add minimal code in core):

  - Discovery phase start/stop; per-directory timings optional
  - Content ID hashing start/stop and counters: bytes read (actual), files hashed (full vs sampled), hash errors
  - Media extraction: items processed/sec by type
  - DB metrics: entries inserted, batched writes, txn count, avg/percentile txn duration
  - Global wall-clock timings per phase and total

- **Event schema (NDJSON)**
  - `bench_meta`: env (hardware, OS), git commit, rustc, features
  - `phase_start` / `phase_end`: phase name, timestamp
  - `counter`: name, value, unit, at timestamp
  - `summary`: computed metrics (files/sec, MB/sec, p50/p90/p99 latencies)

All outputs are machine-readable first; human-friendly markdown is derived from JSON.

---

### 7) Methodology & Repeatability

- **Runs**: Default 5–10 runs per scenario; report median ± MAD (or SD). Persist all raw runs.
- **Caches**: For Linux, instruct dropping caches between cold runs (requires sudo; optional). For macOS, document lack of reliable page cache flush; report both first (cold-ish) and subsequent (warm) run medians.
- **Isolation**: Advise disabling Spotlight/Indexing and background heavy apps; pin CPU governor where applicable.
- **Concurrency**: Fix worker counts where relevant to avoid run-to-run drift.
- **Data locality**: Ensure datasets reside on the intended storage (NVMe vs HDD vs network share). For remote, record provider throttles/limits.

---

### 8) Scenarios Matrix (initial set)

- Local storage:

  - NVMe: discovery-only, discovery+content, full (with media off/on)
  - External SSD (USB 3.2): same as above
  - HDD (USB 3.0/SATA): same as above

- Network storage:

  - NAS over 1Gbps (and optionally 10Gbps): discovery-only, discovery+content

- Remote providers:

  - S3 (metadata-only; optional content sampling via ranged reads)
  - Google Drive (metadata-only)
  - FTP/SFTP/WebDAV (local container targets for reproducibility)

- Search & DB:
  - Keyword and semantic search at 1M entries: p50/p90/p99
  - Bulk ingest (DB write throughput) using generated Entry batches

---

### 9) Reporting & Publication

- Store raw results in `benchmarks/results/` with timestamped filenames.
- `sd-bench report` produces:
  - Markdown summary (`docs/benchmarks.md`) including environment details and scenario tables
  - CSV exports for spreadsheet analysis
  - Optional JSON-to-plot script (e.g., gnuplot/vega spec) for charts

Version every published report with git commit hashes and recipe checksums.

---

### 10) CI, Regression Tracking, and Guardrails

- CI runs micro-benchmarks only (hashing, DB ingest on tiny datasets) to avoid long jobs.
- Nightly/weekly scheduled benchmarks on dedicated hardware (self-hosted runners) produce artifacts and trend lines.
- Introduce threshold alerts: if median files/sec drops >X% vs last baseline, open an issue automatically.

---

### 11) Privacy, Licensing, and Safety

- Synthetic datasets by default; no personal data.
- Public corpora scripts include license notices and checksums.
- Remote benchmarks authenticate via env vars and redact from results.

---

### 12) Implementation Plan (phased)

1. Scaffold `benchmarks/` crate with `sd-bench` CLI; define result schemas.
2. Add minimal core instrumentation (per-phase timers/counters) behind a feature flag `bench_mode`.
3. Implement `mkdata` generator with YAML recipes; produce multi-Location directory trees.
4. Implement `run indexing-…` scenarios for local storage; emit NDJSON/JSON.
5. Add `report` to render markdown summaries and CSV.
6. Extend to search and DB ingest benchmarks.
7. Add remote/provider scenarios (MinIO, containers for FTP/SFTP/WebDAV); optional GDrive.
8. Add weekly scheduled runner and doc publishing.

Deliverables per milestone include: code, example recipes, baseline results, and an updated `docs/benchmarks.md`.

---

### 13) Open Questions

- Exact instrumentation points in current indexing phases (`src/operations/indexing/phases/…`): finalize names and ownership.
- How we want to toggle DB persistence and PRAGMAs for “discovery-only” comparative runs.
- Which media fixtures to include by default (balance between realism and runtime).
- Do we want a small “golden” dataset versioned in the repo purely for CI sanity checks?

---

### 14) What we need from you (Test Locations)

If you can create and maintain recipe YAMLs for canonical datasets (NVMe-small, NVMe-mixed, SSD-mixed, HDD-large, NAS-1G, NAS-10G, S3-metadata-only, etc.), we’ll wire the generator to build them locally into `benchdata/…` and optionally mirror to remote targets. Include:

- Desired total file counts and size distributions
- Directory depth/fanout
- Media ratios and which types to generate
- Duplicate ratios
- Any special path patterns you want (e.g., deep nested trees, many small dirs)

This design supports evolving datasets without checking in large files and lets us replicate results across machines.
