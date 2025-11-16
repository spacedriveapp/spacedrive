# Log Analyzer

A powerful log analysis system for Spacedrive that parses, patterns, and collapses repetitive log entries into queryable insights.

## Features

- **Pattern Detection**: Automatically identifies log templates and variables
- **Collapse Repetitions**: Groups consecutive similar logs with statistics
- **Sequence Detection**: Identifies repeating multi-step patterns (e.g., Commit → Emit × 1000)
- **Phase-Based Summary**: Aggregates activity by time windows (e.g., "5s intervals showing operation counts")
- **Condensed Timeline**: Collapses sequences while preserving chronological order
- **Queryable Storage**: SQLite database for advanced queries with sequences table
- **Timeline Generation**: Visual timeline of log activity
- **Multiple Output Formats**: Markdown reports, JSON export, and phase summaries
- **Extreme Compression**: 99.5% compression achieved on real Spacedrive logs (73,028 lines → 398 sequences)

## Quick Start

### Recommended: Phase Summary

For most debugging tasks, start with the phase summary:

```bash
cargo run --example analyze_sync_log --features cli -- phases test.log --duration 5
```

This aggregates 85,000+ interleaved log lines into 5-second windows showing operation counts, perfect for understanding sync flow.

### As a Library

```rust
use log_analyzer::LogAnalyzer;

let analyzer = LogAnalyzer::from_file("test.log")?;

// Phase-based summary (most useful)
let summary = analyzer.generate_phase_summary(5)?;  // 5s windows
println!("{}", summary);

// Statistics
println!("Compression: {:.1}%", analyzer.compression_ratio() * 100.0);
```

### As a CLI Tool

```bash
# Phase summary (recommended - shows "what happened when")
cargo run --example analyze_sync_log --features cli -- phases test.log --duration 5

# Statistics (quick overview)
cargo run --example analyze_sync_log --features cli -- stats test.log

# Condensed timeline (preserves chronological order)
cargo run --example analyze_sync_log --features cli -- condense test.log

# Full analysis with markdown report
cargo run --example analyze_sync_log --features cli -- analyze test.log

# Export to JSON with database
cargo run --example analyze_sync_log --features cli -- analyze test.log \
    --format json \
    --output analysis.json \
    --database analysis.db
```

## How It Works

### 1. Parse

Extracts structured components from each log line:

```
2025-11-16T07:19:57.232531Z DEBUG ThreadId(02) sd_core::sync::peer: Recorded ACK peer=1817e146
                 ↓
{
  timestamp: 2025-11-16T07:19:57.232531Z,
  level: DEBUG,
  thread: ThreadId(02),
  module: "sd_core::sync::peer",
  message: "Recorded ACK peer=1817e146"
}
```

### 2. Pattern

Identifies templates by comparing similar messages:

```
Log A: "Recorded ACK peer=1817e146"
Log B: "Recorded ACK peer=8ef7a321"
         ↓
Template: "Recorded ACK peer={UUID}"
```

### 3. Collapse

Groups consecutive instances with statistics:

```
Template: "Recorded ACK peer={UUID}"
Count: 1992
Duration: 96ms
Variables:
  - UUID: 1992 unique values
```

### 4. Detect Sequences

Identifies repeating multi-step patterns:

```
Pattern: [Commit, Emit Event]
Detected: This 2-step sequence repeats 1000× times
Result: 2000 groups → 1 sequence (compression!)
```

### 5. Phase-Based Summary (NEW - Most Useful!)

Aggregates operations by time windows for high-level flow understanding:

```bash
cargo run --example analyze_sync_log --features cli -- phases test.log --duration 5
```

**Output:**

```
## 09:15:28 → 09:15:33 (5s, 12,450 events)

### Key Operations

[4,028×] transaction: Committing device-owned data (entry)
[3,829×] sync_transport: [MockTransport] Delivering message to target
[3,829×] peer: State change applied successfully (entry)
[2,015×] peer: Broadcasting shared change (content_identity)
  [1,936×] peer: Shared change applied successfully
  [  200×] dependency: Added dependency tracking

### By Module

  sd_core::service::sync: 8,450 events
  sd_core::infra::sync: 2,100 events
  sync_realtime_integration_test::helpers: 1,900 events
```

**Benefits:**

- Shows "what happened when" without chronological noise
- Highlights warnings/errors per phase
- Aggregates thousands of interleaved async operations
- Perfect for understanding sync flow at a glance

### 6. Store & Query

Save to SQLite for advanced queries:

```sql
-- Query sequences
SELECT id, repetitions, description, group_count
FROM sequences
ORDER BY repetitions DESC;

-- Query templates
SELECT template_id, COUNT(*) as count
FROM log_instances
WHERE timestamp BETWEEN '07:19:57' AND '07:20:00'
GROUP BY template_id;
```

## Architecture

```
Log File → Parser → Pattern Detector → Collapse Engine → Sequence Detector → Database
                                                                 ↓
                                                       Analysis & Reports
```

### Analysis Pipeline

1. **Parser**: Extract structured log components (73,028 logs)
2. **Pattern Detector**: Identify templates (102 unique patterns)
3. **Collapse Engine**: Group consecutive repetitions (64,709 groups)
4. **Sequence Detector**: Find repeating multi-step patterns (398 sequences)
5. **Result**: 99.5% compression ratio!

## Examples

See `examples/` directory:

- `simple.rs` - Basic library usage
- `analyze_sync_log.rs` - Full-featured CLI tool

## Testing

```bash
cargo test -p log-analyzer
```

## Performance

Real-world results from Spacedrive sync test logs:

- **Input**: 73,028 log lines (40MB+)
- **Parsing**: ~1 second
- **Templates**: 102 unique patterns detected
- **Groups**: 64,709 collapsed groups
- **Sequences**: 398 multi-step patterns
- **Compression**: 99.5% (73,028 → 398)

Example sequences detected:

- 1000× "Commit → Emit Event" (2-step pattern)
- 790× "Track Dependency → Add Tracking" (2-step pattern)
- 499× "Commit → Emit → Commit → Emit" (4-step pattern)

## Output Modes

### 1. Phase Summary (Best for Understanding Flow)

Shows aggregated operations per time window - answers "what happened when?"

```
## 09:15:37 → 09:15:42 (5s, 46,581 events)

[5,374×] Delivering message to target
[3,576×] State change sent successfully
[1,798×] Shared change applied successfully
   [1,798×] Broadcasting shared change
   [1,798×] ACK sent successfully
```

**Use when:** Debugging sync flow, understanding system behavior

### 2. Condensed Timeline (Best for Chronological View)

Shows collapsed sequences preserving order - answers "in what sequence?"

```
[999× SEQUENCE] 09:15:28 → 09:15:38 (2 steps, 10s total)
  Step 1: Committing device-owned data (entry)
  Step 2: Sync event emitted (StateChange)

09:15:38 Location created successfully
[100×] 09:15:38 Indexing jobs still running (200ms)
```

**Use when:** Timeline analysis, event ordering matters

### 3. Full Report (Best for Documentation)

Markdown/JSON with complete statistics and templates

**Use when:** Sharing analysis, generating documentation

## Use Cases

### Sync Test Debugging

Instead of manually scrolling through 85,000 log lines, get:

**Phase Summary:**

- See that 4,028 entries committed in 09:15:28-33 window
- See that 3,829 applied successfully (239 missing = stuck in dependencies)
- Spot 24 warnings in 09:15:37-42 window

**Condensed Timeline:**

- 999× "Commit→Emit" sequence collapsed to 1 line
- Easy anomaly spotting: Missing steps in expected sequences

### Performance Analysis

- Measure throughput: "20,750 operations/sec"
- Identify bottlenecks: Sequences with long durations
- Compare test runs: Query database for regression analysis

### CI Integration

- Automated log verification in tests
- Assert expected sequence patterns
- Fail builds on missing or broken sequences

### Example Query

```sql
-- Find the most frequently repeating sequences
SELECT
    s.id,
    s.repetitions,
    s.description,
    s.group_count,
    s.template_sequence
FROM sequences s
ORDER BY s.repetitions DESC
LIMIT 10;
```

## Design

See `/docs/core/design/LOG_ANALYSIS_SYSTEM.md` for complete design specification.
