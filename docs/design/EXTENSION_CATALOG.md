# Spacedrive Extension Catalog

**Date:** October 11, 2025
**Status:** Complete Extension Lineup for November 2025 Launch

This document outlines **all planned extensions** for the Spacedrive ecosystem, demonstrating the platform's versatility from professional tools to personal data archival.

---

## Extension Philosophy

### The Two Extension Types

**1. Paid Professional Extensions** (Business Logic + Intelligence)
- Add domain-specific intelligence (AI agents, specialized analysis)
- Create new workflows and capabilities
- Examples: Chronicle (research), Atlas (CRM), Cipher (security)

**2. Open Source Archive Extensions** (Data Hoarding + Backup)
- Import and preserve data from external services
- Make that data queryable in Spacedrive
- Other extensions can consume this archived data
- Examples: Email Archive, Tweet Archive, Spotify History

### Cross-Extension Dependencies

Extensions can depend on and consume each other's data:

```rust
// Email Archive extension provides
#[model]
#[scope = "standalone"]
#[shared_model]  // ← Other extensions can use this!
struct Email {
    from: String,
    to: Vec<String>,
    subject: String,
    body: String,
    received_at: DateTime<Utc>,
}

// Atlas (CRM) extension depends on Email Archive
#[extension(
    dependencies = ["email_archive"]  // ← Declares dependency
)]
struct Atlas;

// Atlas can query Email models
let emails_from_client = ctx.vdfs()
    .query_models::<Email>()  // Email defined by email_archive extension
    .where_field("from", contains("client@company.com"))
    .collect()
    .await?;
```

**Shared Model Registry:** Core tracks which extensions export models for others to use.

---

## Paid Extensions (Closed Source)

### 1. Chronicle - Research & Knowledge Management

**Purpose:** AI-powered research assistant for knowledge workers

**Price:** $10/mo, $200 lifetime
**Status:** Open source (flagship)
**Market:** Knowledge management ($28B)

#### Models

```rust
// Content-scoped
#[model(scope = "content")]
struct DocumentAnalysis {
    summary: String,
    key_concepts: Vec<String>,
    citations: Vec<Citation>,
    reading_time_minutes: u32,
}

// Standalone
#[model(scope = "standalone")]
struct ResearchProject {
    name: String,
    content_ids: Vec<Uuid>,  // PDFs, notes, web pages
    knowledge_graph: Vec<ConceptLink>,
}

#[model]
struct Concept {
    name: String,
    definition: String,
    related_docs: Vec<Uuid>,
}

#[model]
struct Note {
    title: String,
    body: String,  // Markdown
    linked_docs: Vec<Uuid>,
}
```

#### Jobs

- `analyze_document` - Extract summary, concepts, citations
- `build_knowledge_graph` - Create concept relationships
- `find_research_gaps` - Identify missing knowledge
- `generate_reading_list` - Suggest papers based on gaps

#### Actions

- `create_project` - New research project from selection
- `add_to_project` - Add document to existing project
- `generate_summary` - AI summary of selected docs
- `export_bibliography` - Generate citations file

#### Agent

```rust
#[agent]
impl Chronicle {
    #[on_event(EntryCreated)]
    #[filter = ".of_type::<Pdf>()"]
    async fn on_new_paper(entry: Entry, ctx: &AgentContext) {
        // Analyze document, extract concepts, update knowledge graph
    }

    #[on_query("what am I missing on {topic}")]
    async fn find_gaps(topic: String) -> Vec<Paper> {
        // Compare user's papers to canonical literature
    }
}
```

---

### 2. Ledger - Financial Intelligence

**Purpose:** Receipt extraction, expense tracking, tax preparation

**Price:** $8/mo, $150 lifetime
**Status:** Closed source
**Market:** Expense management ($8B)

#### Models

```rust
// Content-scoped
#[model(scope = "content")]
struct ReceiptAnalysis {
    merchant: String,
    amount: Decimal,
    currency: String,
    date: NaiveDate,
    category: Category,
    items: Vec<LineItem>,
    tax_amount: Option<Decimal>,
}

// Standalone
#[model]
struct Budget {
    category: Category,
    monthly_limit: Decimal,
    current_spending: Decimal,
}

#[model]
struct TaxDocument {
    year: u32,
    receipts: Vec<Uuid>,  // Content UUIDs
    deductions: Vec<Deduction>,
    estimated_savings: Decimal,
}
```

#### Jobs

- `scan_for_receipts` - OCR photos/PDFs for receipt patterns
- `extract_receipt_data` - Parse merchant, amount, date
- `categorize_expenses` - Auto-categorize based on merchant
- `generate_tax_summary` - Compile deductible expenses
- `import_bank_statements` - Parse CSV/PDF statements

#### Actions

- `mark_deductible` - Flag expense for taxes
- `set_budget` - Create monthly budget
- `split_expense` - Multi-category split
- `export_for_quickbooks` - Generate export file

#### Agent

```rust
#[agent]
impl Ledger {
    #[on_event(EntryCreated)]
    #[filter = ".matches_receipt_pattern()"]
    async fn on_potential_receipt(entry: Entry) {
        // Auto-detect and extract receipt data
    }

    #[scheduled(cron = "0 0 1 * *")]  // Monthly
    async fn budget_check() {
        // Alert if over budget, suggest optimizations
    }
}
```

**Cross-Extension:** Can consume Email Archive to extract receipts from Gmail.

---

### 3. Atlas - Dynamic CRM & Team Knowledge

**Purpose:** Customizable CRM with dynamic schemas

**Price:** $30/mo individual, enterprise licensing
**Status:** Closed source, in production internally
**Market:** CRM for small business ($8.2B)

#### Models

```rust
// Runtime-defined schemas!
#[model]
#[runtime_schema]  // User defines fields at runtime
struct Contact {
    // Base fields
    name: String,
    email: String,

    // Runtime fields (stored in models.data)
    custom_fields: HashMap<String, JsonValue>,
}

#[model]
struct Company {
    name: String,
    domain: String,
    contacts: Vec<Uuid>,
}

#[model]
struct Deal {
    title: String,
    value: Decimal,
    stage: Stage,
    contact_id: Uuid,
    related_docs: Vec<Uuid>,  // Content UUIDs
}

#[model]
struct Interaction {
    contact_id: Uuid,
    type_: InteractionType,  // Email, Call, Meeting
    date: DateTime<Utc>,
    notes: String,
    related_entries: Vec<Uuid>,
}
```

#### Jobs

- `import_contacts` - From Gmail, Outlook, LinkedIn
- `extract_business_cards` - OCR from photos
- `analyze_interactions` - Parse email threads for context
- `generate_follow_ups` - Identify stale relationships

#### Actions

- `create_custom_field` - Runtime schema evolution
- `create_pipeline_view` - Kanban board for deals
- `log_interaction` - Record call/meeting
- `share_with_team` - Multi-user collaboration

#### Agent

```rust
#[agent]
impl Atlas {
    #[on_event(Email::Received)]  // Depends on Email Archive
    async fn on_new_email(email: Email) {
        // Extract contact, log interaction, update deal stage
    }

    #[on_query("who should I follow up with")]
    async fn suggest_followups() -> Vec<Contact> {
        // Find contacts not contacted in 90 days
    }
}
```

**Cross-Extension:** Consumes Email Archive, can query Ledger for deal values.

---

### 4. Cipher - Security & Encryption

**Purpose:** Password manager + file encryption + key management

**Price:** $8/mo, $150 lifetime
**Status:** Closed source
**Market:** Password management ($2.5B)

#### Models

```rust
#[model]
struct Vault {
    name: String,
    master_password_hash: String,
    unlock_method: UnlockMethod,  // Biometric, Password, Hardware Key
}

#[model]
struct Credential {
    vault_id: Uuid,
    name: String,
    username: String,

    #[blob_data(compression = "none", encrypted = true)]
    password: String,  // Encrypted at rest

    url: Option<String>,
    totp_secret: Option<String>,
    notes: Option<String>,
    tags: Vec<String>,
}

#[model(scope = "entry")]  // Tied to specific file
struct EncryptedFile {
    entry_uuid: Uuid,
    encryption_method: String,
    key_vault_id: Uuid,
}
```

#### Jobs

- `scan_weak_passwords` - Check password strength
- `check_breaches` - Compare against HaveIBeenPwned
- `encrypt_folder` - Bulk file encryption
- `backup_vault` - Export encrypted backup

#### Actions

- `generate_password` - Create strong password
- `encrypt_selection` - Encrypt selected files
- `unlock_vault` - Decrypt vault for session
- `share_credential` - Share with team (encrypted)

#### Agent

```rust
#[agent]
impl Cipher {
    #[on_event(CredentialCreated)]
    async fn on_new_credential(cred: Credential) {
        // Check breach databases, validate strength
    }

    #[scheduled(cron = "0 0 * * *")]  // Daily
    async fn security_audit() {
        // Check weak passwords, unused credentials
    }
}
```

**Cross-Extension:** Provides key management for other extensions' encryption needs.

---

### 5. Studio - Digital Asset Management

**Purpose:** Professional media management for creators

**Price:** $15/mo
**Status:** Closed source
**Market:** DAM software ($4.5B)

#### Models

```rust
// Content-scoped
#[model(scope = "content")]
struct VideoAnalysis {
    scenes: Vec<SceneMarker>,
    transcript: String,
    speakers: Vec<String>,
    topics: Vec<String>,
}

// Standalone
#[model]
struct Project {
    name: String,
    assets: Vec<Uuid>,  // Content UUIDs
    type_: ProjectType,  // Video, Photo, Audio
}

#[model]
struct AssetVersion {
    original_content_id: Uuid,
    version_number: u32,
    edits: Vec<Edit>,
    exported_content_id: Option<Uuid>,
}
```

#### Jobs

- `transcode_video` - Generate proxies for editing
- `extract_scenes` - Auto-detect scene changes
- `transcribe_audio` - Speech-to-text for videos
- `generate_subtitles` - Create SRT files

#### Actions

- `create_project` - New media project
- `mark_scene` - Add scene marker
- `export_project` - Generate deliverable
- `share_for_review` - Collaborative review

#### Agent

```rust
#[agent]
impl Studio {
    #[on_event(VideoImported)]
    async fn on_new_video(video: Entry) {
        // Auto-transcode, extract scenes, generate thumbnails
    }
}
```

---

## Open Source Archive Extensions

### 6. Email Archive

**Purpose:** Local backup of all emails (Gmail, Outlook, etc.)

**Price:** Free, open source
**Why Free:** Drives adoption, other extensions consume this data

#### Models

```rust
#[model]
#[scope = "standalone"]
#[shared_model]  // ← Other extensions can use Email model!
struct Email {
    message_id: String,
    from: String,
    to: Vec<String>,
    cc: Vec<String>,
    subject: String,

    #[blob_data]
    body_html: String,
    body_text: String,

    received_at: DateTime<Utc>,
    attachments: Vec<Uuid>,  // Content UUIDs
    thread_id: Option<String>,
}

#[model]
struct EmailAccount {
    provider: Provider,  // Gmail, Outlook, IMAP
    email_address: String,
    last_sync: DateTime<Utc>,
    total_emails: usize,
}
```

#### Jobs

- `sync_gmail` - Incremental sync via Gmail API
- `sync_imap` - Generic IMAP sync
- `download_attachments` - Save attachments as entries
- `index_threads` - Build conversation threads

#### Agent

```rust
#[agent]
impl EmailArchive {
    #[scheduled(cron = "0 */4 * * *")]  // Every 4 hours
    async fn incremental_sync() {
        // Sync new emails from all accounts
    }
}
```

**Consumed By:** Atlas (CRM), Chronicle (research emails), Ledger (receipt extraction)

---

### 7. Chrome History

**Purpose:** Archive browsing history, bookmarks, downloads

**Price:** Free, open source

#### Models

```rust
#[model]
#[shared_model]
struct BrowsingHistory {
    url: String,
    title: String,
    visited_at: DateTime<Utc>,
    visit_count: u32,

    #[blob_data(lazy = true)]
    page_content: Option<String>,  // Optional archived HTML
}

#[model]
struct Bookmark {
    url: String,
    title: String,
    folder: String,
    created_at: DateTime<Utc>,
}

#[model(scope = "content")]
struct DownloadMetadata {
    source_url: String,
    download_date: DateTime<Utc>,
    referrer: Option<String>,
}
```

#### Jobs

- `import_chrome_history` - Parse Chrome SQLite DB
- `archive_bookmarks` - Import bookmarks
- `download_pages` - Save webpage snapshots
- `link_downloads_to_sources` - Associate downloads with URLs

**Consumed By:** Chronicle (research sources), Atlas (client websites)

---

### 8. Spotify Archive

**Purpose:** Complete listening history and library backup

**Price:** Free, open source

#### Models

```rust
#[model]
#[shared_model]
struct ListeningHistory {
    track_id: String,
    track_name: String,
    artist: String,
    album: String,
    played_at: DateTime<Utc>,
    duration_ms: u32,
    shuffle: bool,
}

#[model]
struct Playlist {
    name: String,
    tracks: Vec<TrackId>,
    created_at: DateTime<Utc>,
    is_public: bool,
}
```

#### Jobs

- `sync_spotify_history` - Via Spotify API
- `backup_playlists` - Save playlist data
- `download_liked_songs` - Optional local copies

**Consumed By:** Could inspire music-focused extensions, personal analytics

---

### 9. GPS Location History

**Purpose:** Track your location over time (privacy-first alternative to Google Timeline)

**Price:** Free, open source

#### Models

```rust
#[model]
#[shared_model]
struct LocationPoint {
    latitude: f64,
    longitude: f64,
    accuracy_meters: f32,
    timestamp: DateTime<Utc>,
    activity: Option<Activity>,  // Walking, Driving, Still
}

#[model]
struct Visit {
    place_id: Option<Uuid>,  // Links to Photos Place if available
    arrived_at: DateTime<Utc>,
    departed_at: DateTime<Utc>,
    location: GpsCoordinates,
}
```

#### Jobs

- `import_google_timeline` - Import existing Google data
- `sync_ios_location` - From iOS location services
- `detect_places` - Cluster visits into places
- `correlate_with_photos` - Match photo GPS to timeline

**Consumed By:** Photos (correlate location with photos), personal analytics

---

### 10. Tweet Archive

**Purpose:** Complete Twitter/X archive and backup

**Price:** Free, open source

#### Models

```rust
#[model]
#[shared_model]
struct Tweet {
    tweet_id: String,
    text: String,
    author: String,
    created_at: DateTime<Utc>,
    likes: u32,
    retweets: u32,
    replies: u32,
    media: Vec<Uuid>,  // Content UUIDs for images/videos
}

#[model]
struct TwitterAccount {
    username: String,
    followers: u32,
    following: u32,
    last_sync: DateTime<Utc>,
}
```

#### Jobs

- `import_twitter_archive` - From Twitter data export
- `sync_via_api` - Incremental sync
- `download_media` - Save tweet images/videos
- `analyze_engagement` - Track tweet performance

**Consumed By:** Chronicle (tweets as research sources), personal analytics

---

### 11. GitHub Repo Tracker

**Purpose:** Track repositories, stars, contributions

**Price:** Free, open source

#### Models

```rust
#[model]
#[shared_model]
struct Repository {
    full_name: String,
    description: String,
    stars: u32,
    language: String,
    topics: Vec<String>,
    last_commit: DateTime<Utc>,
}

#[model]
struct Contribution {
    repo: String,
    date: DateTime<Utc>,
    commits: u32,
    additions: u32,
    deletions: u32,
}
```

#### Jobs

- `sync_starred_repos` - Track starred repos
- `track_contributions` - Your commit history
- `clone_repos_locally` - Backup source code
- `analyze_activity` - Contribution patterns

**Consumed By:** Chronicle (code documentation), personal analytics

---

## Cross-Extension Synergies

### Email Archive → Multiple Extensions

```rust
// Email Archive provides base Email model
#[shared_model]
struct Email { from, to, subject, body, ... }

// Ledger consumes for receipts
#[job]
async fn extract_receipts_from_email(ctx: &JobContext) -> JobResult<()> {
    let emails = ctx.vdfs()
        .query_models::<Email>()  // From Email Archive
        .where_field("subject", contains("receipt"))
        .collect()
        .await?;

    for email in emails {
        if contains_receipt_pattern(&email.body) {
            let receipt = extract_receipt_data(&email)?;
            ctx.vdfs().create_model(receipt).await?;
        }
    }

    Ok(())
}

// Atlas consumes for CRM
#[agent]
impl Atlas {
    async fn on_email_received(email: Email) {
        // Find or create contact, log interaction
    }
}

// Chronicle consumes for research
// "Show me all emails about machine learning"
```

### Photos + GPS Location History

```rust
// GPS provides location timeline
#[shared_model]
struct LocationPoint { lat, lon, timestamp }

// Photos consumes to enrich
#[job]
async fn correlate_photos_with_timeline(ctx: &JobContext) -> JobResult<()> {
    let photos = ctx.vdfs().query_models::<PhotoAnalysis>().collect().await?;
    let timeline = ctx.vdfs().query_models::<LocationPoint>().collect().await?;

    for photo in photos {
        // Match photo timestamp to location timeline
        if let Some(location) = find_closest_location(&photo, &timeline) {
            // Enhance photo with precise location even if GPS missing
        }
    }

    Ok(())
}
```

### Chrome History + Chronicle

```rust
// Chronicle uses browsing history for context
#[agent]
impl Chronicle {
    async fn suggest_research_sources(topic: String) -> Vec<Source> {
        // Check browsing history for relevant sites
        let relevant_urls = ctx.vdfs()
            .query_models::<BrowsingHistory>()  // From Chrome extension
            .search_semantic("url", similar_to(&topic))
            .collect()
            .await?;

        // Suggest archiving these pages for research
    }
}
```

---

## Shared Model Registry System

### How Extension Dependencies Work

```rust
// Email Archive extension manifest
{
  "id": "email_archive",
  "exports": {
    "models": ["Email", "EmailAccount"],
    "queries": ["search_emails", "get_thread"]
  }
}

// Atlas extension manifest
{
  "id": "atlas",
  "dependencies": ["email_archive"],
  "imports": {
    "email_archive": {
      "models": ["Email"],
      "queries": ["search_emails"]
    }
  }
}
```

### Runtime Model Resolution

```rust
// Core tracks exported models
pub struct SharedModelRegistry {
    exports: HashMap<String, Vec<ModelSpec>>,  // extension_id → models
}

// When Atlas queries Email
ctx.vdfs().query_models::<Email>()

// Core resolves:
// 1. Check if Email is defined by Atlas → No
// 2. Check Atlas dependencies → email_archive
// 3. Check if email_archive exports Email → Yes
// 4. Query: SELECT * FROM models WHERE extension_id = 'email_archive' AND model_type = 'Email'
```

### Versioning & Compatibility

```rust
// Email Archive v1.0
#[model(version = "1.0.0")]
#[shared_model]
struct Email {
    from: String,
    subject: String,
    body: String,
}

// Email Archive v2.0 adds field
#[model(version = "2.0.0")]
#[migrate_from = "1.0.0"]
struct Email {
    from: String,
    subject: String,
    body: String,
    priority: Option<Priority>,  // New field
}

// Dependent extensions declare compatible versions
#[extension(
    dependencies = [
        ("email_archive", "^1.0")  // SemVer - works with 1.x
    ]
)]
```

---

## Extension Lineup Summary

### Paid Extensions (Revenue Generating)

| Extension | Price | Market | Key Feature | Agent Capability |
|-----------|-------|--------|-------------|------------------|
| Chronicle | $10/mo | $28B | Knowledge graphs | Research gaps |
| Ledger | $8/mo | $8B | Receipt OCR | Budget alerts |
| Atlas | $30/mo | $8.2B | Dynamic schemas | Relationship tracking |
| Cipher | $8/mo | $2.5B | Zero-knowledge | Breach monitoring |
| Studio | $15/mo | $4.5B | Scene detection | Project automation |

**Total Addressable Market:** $51.2B annually

### Open Source Archives (Adoption Drivers)

| Extension | Purpose | Provides | Consumed By |
|-----------|---------|----------|-------------|
| Email Archive | Gmail/Outlook backup | Email model | Atlas, Ledger, Chronicle |
| Chrome History | Browsing backup | URLs, bookmarks | Chronicle |
| Spotify | Listening history | Tracks, playlists | Analytics |
| GPS Tracker | Location timeline | Location points | Photos, Analytics |
| Tweet Archive | Twitter backup | Tweets | Chronicle, Analytics |
| GitHub Tracker | Repo tracking | Repos, commits | Chronicle |

---

## Launch Strategy

### Phase 1: November 2025 (Alpha)

**Launch with:**
1. Chronicle (open source flagship)
2. Cipher (universal appeal)
3. Ledger (personal productivity)
4. Atlas (enterprise credibility)

**Plus 2 archive extensions:**
5. Email Archive (drives Atlas/Ledger adoption)
6. Chrome History (drives Chronicle adoption)

**Bundles:**
- Personal: Chronicle + Cipher + Ledger = $20/mo or $400 lifetime
- Enterprise: Atlas = $30/mo

### Phase 2: Q1 2026

7. Studio (creator market)
8. GPS Location History
9. Spotify Archive
10. Tweet Archive

### Phase 3: Q2 2026

11. GitHub Tracker
12. Third-party marketplace opens
13. Community extensions

---

## The Extension Ecosystem Value

### For Users

**Data Hoarding Made Useful:**
- Archive everything (emails, tweets, locations, browsing)
- Free, open source archival extensions
- Paid extensions add intelligence on top

**Example User Journey:**
1. Install Email Archive (free) → backs up Gmail
2. Install Ledger ($8/mo) → automatically extracts receipts from emails
3. Value: Receipts extracted from 10 years of email history instantly

### For Platform

**Archive Extensions as Growth Engine:**
- Free extensions drive adoption
- Demonstrate platform capabilities
- Create data gravity (more data in Spacedrive)
- Upsell path to paid extensions

**Chronicle Depends on Archives:**
- Better with Email Archive (research emails)
- Better with Chrome History (web sources)
- Better with Tweet Archive (social research)

### For Developers

**Shared Model System:**
- Don't reinvent Email model
- Import from email_archive extension
- Focus on your domain logic
- Extensions become composable

---

## Technical: Shared Model System

### Registry Schema

```sql
CREATE TABLE extension_exports (
    extension_id TEXT NOT NULL,
    export_type TEXT NOT NULL,  -- "model", "query", "action"
    export_name TEXT NOT NULL,
    version TEXT NOT NULL,
    schema TEXT NOT NULL,  -- JSON schema

    PRIMARY KEY (extension_id, export_type, export_name)
);
```

### Model Compatibility Check

```rust
impl Core {
    fn validate_extension_dependencies(
        &self,
        extension_id: &str,
        dependencies: &[Dependency],
    ) -> Result<()> {
        for dep in dependencies {
            // Check if dependency installed
            if !self.is_installed(&dep.extension_id) {
                return Err(MissingDependency(dep.extension_id));
            }

            // Check version compatibility
            let installed_version = self.get_version(&dep.extension_id)?;
            if !dep.version_req.matches(&installed_version) {
                return Err(IncompatibleVersion {
                    required: dep.version_req,
                    installed: installed_version,
                });
            }

            // Check exported models match
            for model in &dep.required_models {
                let export = self.get_export(&dep.extension_id, "model", model)?;
                // Verify schema compatibility
            }
        }

        Ok(())
    }
}
```

### Installation Order

```
User installs Atlas (depends on email_archive)
  ↓
Core checks: email_archive installed? → No
  ↓
Prompt: "Atlas requires Email Archive. Install it first?"
  ↓
Install email_archive → Install atlas
  ↓
Both running, Atlas can query Email models
```

---

## Business Model Benefits

### Free Archives Drive Paid Extensions

```
User Journey:
1. Install Spacedrive (free)
2. Install Email Archive (free) - "Back up my Gmail? Sure!"
3. 10,000 emails archived over a month
4. Install Ledger ($8/mo) - "Found 500 receipts in your email!"
5. User converts: Free archive → Paid intelligence

Conversion funnel:
- Email Archive: 100,000 users (free)
- Ledger: 2,000 users (2% conversion) = $16K MRR
```

### Network Effects

```
More archive extensions → More data in Spacedrive
  ↓
More data → Higher switching costs
  ↓
More users → More extension developers
  ↓
More extensions → More value → More users
```

### Open Source as Marketing

- Email Archive: 50,000 GitHub stars (drives awareness)
- Chronicle: Open source proves extensions can be audited
- Archive extensions: Zero customer acquisition cost (organic)

---

## Development Priorities

### Immediate (November Launch)

**Must Have:**
1. Chronicle (150 lines of business logic)
2. Cipher (200 lines)
3. Ledger (180 lines)
4. Atlas (250 lines - runtime schemas complex)
5. Email Archive (120 lines)
6. Chrome History (100 lines)

**Total:** ~1,000 lines of extension code (SDK does the heavy lifting)

### Q1 2026

7. Studio (300 lines - video is complex)
8. GPS Location (100 lines)
9. Spotify Archive (80 lines)
10. Tweet Archive (90 lines)

### Q2 2026

11. GitHub Tracker (100 lines)
12. Marketplace opens for third-party extensions

---

## Extension Complexity Comparison

| Extension | Business Logic | Why So Small? |
|-----------|---------------|---------------|
| Email Archive | ~120 lines | SDK handles sync, storage, jobs |
| Ledger | ~180 lines | OCR by Core, just parse receipts |
| Chronicle | ~150 lines | Embeddings by Core, just query |
| Atlas | ~250 lines | Schema flexibility adds complexity |

**Compare to building standalone:**
- Email client: 50,000+ lines
- Receipt app: 20,000+ lines
- Research tool: 30,000+ lines

**The SDK is doing the work.** Extensions are pure business logic.

---

## Risk: Extension Conflicts

**Problem:** Two extensions define `Email` model differently.

**Solutions:**

1. **Shared Model Registry** (Recommended)
   - First extension to export `Email` wins
   - Other extensions import it
   - Core enforces schema compatibility

2. **Namespaced Models**
   - `email_archive::Email` vs `atlas::Email`
   - Extensions explicitly import: `use email_archive::Email`
   - No conflicts, but duplication

3. **Core-Defined Common Models**
   - Core defines: Email, Contact, Event, Note
   - Extensions extend with custom fields
   - Guaranteed compatibility

**Recommendation:** Start with shared registry (Solution 1), add core common models later (Solution 3).

---

## Investor Pitch: Extension Lineup

**"Spacedrive launches with 6 extensions in two categories:**

**Professional Intelligence** (Paid):
- Chronicle: AI research assistant ($10/mo, open source)
- Cipher: Password manager + encryption ($8/mo)
- Ledger: Receipt extraction + expense tracking ($8/mo)
- Atlas: Dynamic CRM ($30/mo, in production internally)

**Personal Archives** (Free, open source):
- Email Archive: Complete Gmail/Outlook backup
- Chrome History: Browsing history preservation

**The synergy:** Archive extensions drive adoption and data gravity. Intelligence extensions generate revenue. Both use the same SDK and data primitives.

**Example:** User installs free Email Archive, backs up 20,000 emails. Ledger finds 2,000 receipts automatically. User converts to paid for tax prep features. CAC = $0 (organic from free extension)."

---

**This extension catalog demonstrates Spacedrive as a true platform: Professional tools + personal data ownership + extensible architecture.** 

