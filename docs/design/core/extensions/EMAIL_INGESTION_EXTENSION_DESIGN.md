<!--CREATED: 2025-10-11-->
# Email Ingestion Extension: Technical Design & Prototype

## Executive Summary

This document defines the architecture for Spacedrive's first revenue-generating extension: an email ingestion system that processes receipts and invoices. It bridges the **existing process-based integration system** with the **planned WASM plugin architecture**, providing a practical migration path from MVP to platform.

**Key Decision:** Start with **process-based integration** for rapid MVP development, then refactor to WASM once the platform matures.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Integration Points](#integration-points)
3. [Two-Phase Implementation Strategy](#two-phase-implementation-strategy)
4. [Phase 1: Process-Based MVP](#phase-1-process-based-mvp)
5. [Phase 2: WASM Migration](#phase-2-wasm-migration)
6. [Email Ingestion Pipeline](#email-ingestion-pipeline)
7. [Data Model](#data-model)
8. [Receipt Processing Flow](#receipt-processing-flow)
9. [API Specification](#api-specification)
10. [Prototype Implementation](#prototype-implementation)
11. [Testing Strategy](#testing-strategy)

---

## Architecture Overview

### The Two Approaches

**Process-Based Integration (Ready Now)**
- Separate executable that communicates via IPC
- Can be written in any language (Rust, TypeScript, Python)
- Runs with OS-level isolation
- Fast to prototype, proven pattern

**WASM Plugin (Future)**
- Sandboxed .wasm module loaded by Spacedrive core
- Capability-based security model
- Hot-reloadable, cross-platform single binary
- Requires platform infrastructure (not built yet)

### Decision Matrix

| Criteria | Process-Based | WASM |
|----------|--------------|------|
| **Time to MVP** | ⭐⭐⭐⭐2-3 weeks | 12+ weeks (platform first) |
| **Security** | ⭐⭐OS isolation | ⭐⭐⭐⭐WASM sandbox |
| **Performance** | ⭐⭐IPC overhead | ⭐⭐⭐In-process |
| **Distribution** | ⭐Platform-specific binaries | ⭐⭐⭐⭐Single .wasm |
| **Hot Reload** | ⭐Restart required | ⭐⭐⭐⭐Instant |
| **Debugging** | ⭐⭐⭐⭐Standard tools | ⭐WASM tooling |

**Recommendation:** Ship Phase 1 (process-based) for quick revenue validation, build WASM platform in parallel, migrate in Phase 2.

---

## Integration Points

The email extension integrates with 7 core Spacedrive systems:

### 1. VDFS Entry System
**Purpose:** Represent emails and receipts as Entry records

```rust
// Create Entry for each receipt email
let receipt_entry = Entry {
    id: Uuid::new_v4(),
    path: SdPath::new(device_id, PathBuf::from(format!(
        "~/Library/Spacedrive/extensions/finance/receipts/{}.eml",
        email.message_id
    ))),
    name: format!("Receipt: {} - {}", vendor, date),
    metadata_id: Uuid::new_v4(),
    content_id: Some(ContentId::from_hash(&email_raw_bytes)),
    parent_id: None, // Top-level receipts folder
    discovered_at: Utc::now(),
};
```

**Integration:** Extension calls `VDFS::create_entry()` via IPC

### 2. Virtual Sidecar System
**Purpose:** Store email metadata and AI analysis results

```rust
// Store raw email in sidecar
sidecar_manager.write_sidecar(
    &entry.id,
    "email.json",
    serde_json::to_vec(&EmailMetadata {
        from: email.from,
        to: email.to,
        subject: email.subject,
        date: email.date,
        message_id: email.message_id,
        body_text: email.body_text,
        body_html: email.body_html,
    })?
).await?;

// Store AI-extracted receipt data
sidecar_manager.write_sidecar(
    &entry.id,
    "receipt_analysis.json",
    serde_json::to_vec(&ReceiptData {
        vendor: "Starbucks Coffee",
        amount: 8.47,
        currency: "USD",
        date: "2025-01-15",
        category: "Food & Dining",
        items: vec![
            LineItem { name: "Latte", price: 5.95 },
            LineItem { name: "Croissant", price: 2.52 },
        ],
        tax: 0.68,
        confidence: 0.96,
    })?
).await?;
```

**Integration:** Extension calls `VirtualSidecarSystem::write_sidecar()` via IPC

### 3. Job System
**Purpose:** Durable, resumable email scanning and processing

```rust
// Email scanning job
#[derive(Serialize, Deserialize)]
pub struct EmailScanJob {
    pub last_processed_uid: Option<String>,
    pub processed_count: usize,
    pub total_count: usize,
    pub provider: EmailProvider,

    #[serde(skip)]
    pub credentials: OAuth2Credentials,
}

impl Job for EmailScanJob {
    const NAME: &'static str = "email_scan";
    const RESUMABLE: bool = true;
}

#[async_trait]
impl JobHandler for EmailScanJob {
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<()> {
        let imap_client = connect_imap(&self.credentials).await?;

        // Resume from last processed UID
        let messages = imap_client.fetch_since(
            self.last_processed_uid.as_ref()
        ).await?;

        for msg in messages {
            // Process each message
            if self.is_receipt(&msg) {
                let entry = self.create_receipt_entry(&msg, &ctx).await?;

                // Queue OCR sub-job
                ctx.spawn_sub_job(OcrJob {
                    entry_id: entry.id,
                    attachment_paths: msg.pdf_attachments,
                }).await?;
            }

            // Update progress
            self.processed_count += 1;
            self.last_processed_uid = Some(msg.uid.clone());

            ctx.report_progress(
                self.processed_count as f32 / self.total_count as f32
            ).await?;
        }

        Ok(())
    }
}
```

**Integration:** Extension dispatches jobs via `JobSystem::dispatch()` IPC call

### 4. AI Service
**Purpose:** OCR and receipt classification

```rust
// OCR extraction
let ocr_result = ai_service.ocr(
    &pdf_bytes,
    OcrOptions {
        language: "eng",
        preprocessing: true,
        engine: OcrEngine::Tesseract, // or EasyOCR
    }
).await?;

// AI classification
let receipt_data = ai_service.classify_receipt(
    &ocr_result.text,
    ClassificationOptions {
        model: user_settings.ai_model, // Local Ollama or cloud
        temperature: 0.1,
        structured_output: true,
    }
).await?;
```

**Integration:** Extension calls `AIService::ocr()` and `AIService::classify_receipt()` via IPC

### 5. Credential Manager
**Purpose:** Secure OAuth token storage

```rust
// Store OAuth credentials
credential_manager.store_credential(
    "finance_extension",
    "gmail_oauth",
    CredentialType::OAuth2 {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        scopes: vec!["https://www.googleapis.com/auth/gmail.readonly"],
        expires_at: Utc::now() + Duration::seconds(token_response.expires_in),
    }
).await?;

// Retrieve with auto-refresh
let credentials = credential_manager.get_credential(
    "finance_extension",
    "gmail_oauth"
).await?; // Automatically refreshes if expired
```

**Integration:** Extension calls `CredentialManager::store_credential()` and `get_credential()` via IPC

### 6. Search Index
**Purpose:** Make receipts searchable by natural language

```rust
// After creating entry and sidecar, trigger search indexing
search_service.index_entry(
    &entry.id,
    SearchIndexOptions {
        extract_text: true, // OCR text
        generate_embedding: true, // Semantic search
        index_metadata: true, // Vendor, amount, date
    }
).await?;

// Now users can search:
// "Find receipts from coffee shops last quarter"
// "Show me all restaurant expenses over $50"
```

**Integration:** Automatic via Event Bus (entry created → search index updated)

### 7. Event Bus
**Purpose:** React to system events and trigger processing

```rust
// Extension subscribes to events
event_bus.subscribe("entry.created", |event: EntryCreatedEvent| {
    if event.entry.path.extension() == Some("eml") {
        // Trigger receipt detection
        detect_receipt(event.entry).await;
    }
}).await?;

// Extension publishes events
event_bus.publish("receipt.detected", ReceiptDetectedEvent {
    entry_id: entry.id,
    vendor: receipt_data.vendor,
    amount: receipt_data.amount,
}).await?;
```

**Integration:** Extension subscribes via `EventBus::subscribe()` IPC call

---

## Two-Phase Implementation Strategy

### Phase 1: Process-Based MVP (2-3 weeks)

**Goal:** Validate revenue model with minimal engineering

**Architecture:**
```
┌─────────────────────────────────────────┐
│         Spacedrive Core                 │
│                                         │
│  ┌────────────────────────────────┐    │
│  │   Integration Manager          │    │
│  │                                 │    │
│  │   • Process Launcher           │    │
│  │   • IPC Router                 │    │
│  │   • Credential Manager         │    │
│  └────────────────────────────────┘    │
│                                         │
│  Core Services:                         │
│  • VDFS  • Job System  • AI  • Search  │
└─────────────────────────────────────────┘
                  │
                  │ IPC (Unix Socket / Named Pipe)
                  │
┌─────────────────▼─────────────────────┐
│  Finance Extension (Separate Process) │
│                                        │
│  • Email OAuth Client                 │
│  • IMAP/Gmail API Client              │
│  • Receipt Detection Logic            │
│  • IPC Client Library                 │
└────────────────────────────────────────┘
```

**Deliverables:**
1. `spacedrive-finance` executable
2. IPC protocol implementation
3. Gmail OAuth flow
4. Basic receipt detection
5. Integration with existing core services

**Timeline:** 2-3 weeks for 2 engineers

### Phase 2: WASM Migration (After platform exists)

**Goal:** Better security, distribution, and developer experience

**Architecture:**
```
┌─────────────────────────────────────────┐
│         Spacedrive Core                 │
│                                         │
│  ┌────────────────────────────────┐    │
│  │   WASM Plugin Host              │    │
│  │                                 │    │
│  │   • Wasmer Runtime             │    │
│  │   • VDFS API Bridge            │    │
│  │   • Permission System          │    │
│  │   • Resource Limits            │    │
│  └────────────────────────────────┘    │
│          │                              │
│          │ Direct Function Calls        │
│          │                              │
│  ┌───────▼────────────────────────┐    │
│  │ Finance Plugin (WASM Module)   │    │
│  │                                 │    │
│  │ • Email scanning logic (Rust)  │    │
│  │ • Receipt detection (Rust)     │    │
│  │ • Compiled to .wasm            │    │
│  └─────────────────────────────────┘    │
└─────────────────────────────────────────┘
```

**Migration Path:**
1. Core functionality (receipt detection, classification) stays identical
2. Refactor IPC calls → direct WASM host function calls
3. Package as single `.wasm` file
4. Leverage hot-reload for development

---

## Phase 1: Process-Based MVP

### Project Structure

```
spacedrive-finance/
├── Cargo.toml
├── manifest.json              # Integration metadata
├── src/
│   ├── main.rs                # IPC server & lifecycle
│   ├── lib.rs                 # Core extension logic
│   ├── email/
│   │   ├── mod.rs
│   │   ├── gmail.rs           # Gmail API client
│   │   ├── outlook.rs         # Outlook API client
│   │   ├── imap.rs            # Generic IMAP client
│   │   └── oauth.rs           # OAuth flow helpers
│   ├── receipt/
│   │   ├── mod.rs
│   │   ├── detection.rs       # Heuristics for receipt detection
│   │   ├── extraction.rs      # OCR coordination
│   │   └── classification.rs  # AI classification
│   ├── ipc/
│   │   ├── mod.rs
│   │   ├── client.rs          # IPC client for core API
│   │   ├── server.rs          # IPC server for extension API
│   │   └── protocol.rs        # Message definitions
│   └── jobs/
│       ├── mod.rs
│       ├── scan.rs            # Email scanning job
│       └── process.rs         # Receipt processing job
├── tests/
│   ├── integration_tests.rs
│   └── fixtures/              # Sample emails
└── README.md
```

### manifest.json

```json
{
  "id": "finance",
  "name": "Spacedrive Finance",
  "version": "0.1.0",
  "description": "Receipt and invoice tracking with AI categorization",
  "author": "Spacedrive Technology Inc.",
  "homepage": "https://spacedrive.com/extensions/finance",

  "capabilities": [
    {
      "type": "DataIngestion",
      "sources": ["email"],
      "providers": ["gmail", "outlook", "imap"]
    },
    {
      "type": "ContentProcessor",
      "input_types": ["application/pdf", "image/jpeg", "image/png"],
      "operations": ["ocr", "classification"]
    }
  ],

  "permissions": {
    "network_access": [
      "https://www.googleapis.com",
      "https://graph.microsoft.com",
      "*.imap.gmail.com:993"
    ],
    "core_apis": [
      "vdfs.create_entry",
      "vdfs.write_sidecar",
      "jobs.dispatch",
      "ai_service.ocr",
      "ai_service.classify",
      "credentials.store",
      "credentials.get",
      "search.index"
    ],
    "max_memory_mb": 512,
    "max_cpu_percent": 25
  },

  "configuration_schema": {
    "type": "object",
    "properties": {
      "email_provider": {
        "type": "string",
        "enum": ["gmail", "outlook", "imap"],
        "description": "Email provider to scan"
      },
      "scan_frequency": {
        "type": "string",
        "enum": ["realtime", "hourly", "daily"],
        "default": "hourly"
      },
      "categories": {
        "type": "array",
        "items": { "type": "string" },
        "default": ["Food & Dining", "Transportation", "Office Supplies", "Travel", "Entertainment", "Other"]
      }
    },
    "required": ["email_provider"]
  }
}
```

### IPC Protocol

**Message Format (JSON over Unix Socket):**

```json
// Request from extension to core
{
  "id": "req_123",
  "method": "vdfs.create_entry",
  "params": {
    "name": "Receipt: Starbucks - 2025-01-15",
    "path": "~/Library/Spacedrive/extensions/finance/receipts/msg_456.eml",
    "entry_type": "FinancialDocument",
    "metadata": {
      "vendor": "Starbucks",
      "amount": 8.47,
      "date": "2025-01-15"
    }
  },
  "timeout_ms": 5000
}

// Response from core
{
  "id": "req_123",
  "success": true,
  "data": {
    "entry_id": "550e8400-e29b-41d4-a716-446655440000",
    "created_at": "2025-01-15T10:30:00Z"
  },
  "error": null
}
```

**Rust Implementation:**

```rust
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub id: String,
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    pub async fn connect() -> Result<Self> {
        let socket_path = std::env::var("SPACEDRIVE_IPC_SOCKET")?;
        let stream = UnixStream::connect(socket_path).await?;
        Ok(Self { stream })
    }

    pub async fn request(&mut self, method: &str, params: serde_json::Value) -> Result<IpcResponse> {
        let req = IpcRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
            timeout_ms: Some(5000),
        };

        // Send request
        let req_json = serde_json::to_vec(&req)?;
        let req_len = (req_json.len() as u32).to_be_bytes();
        self.stream.write_all(&req_len).await?;
        self.stream.write_all(&req_json).await?;

        // Read response
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; len];
        self.stream.read_exact(&mut resp_buf).await?;

        let resp: IpcResponse = serde_json::from_slice(&resp_buf)?;
        Ok(resp)
    }
}
```

---

## Email Ingestion Pipeline

### 1. OAuth Setup Flow

**User Experience:**
1. User clicks "Connect Gmail" in Spacedrive Finance settings
2. Extension opens browser to Google OAuth consent screen
3. User authorizes Spacedrive Finance (readonly Gmail access)
4. Extension receives OAuth code and exchanges for tokens
5. Tokens stored in Spacedrive Credential Manager (encrypted)

**Implementation:**

```rust
pub async fn start_gmail_oauth(ipc_client: &mut IpcClient) -> Result<()> {
    // Step 1: Generate OAuth URL
    let oauth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope={}&\
         access_type=offline",
        GMAIL_CLIENT_ID,
        "http://localhost:8765/oauth/callback",
        "https://www.googleapis.com/auth/gmail.readonly"
    );

    // Step 2: Open browser
    open::that(&oauth_url)?;

    // Step 3: Start local server to receive callback
    let (code_tx, code_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:8765").await?;
        let (stream, _) = listener.accept().await?;

        // Parse callback and extract code
        let code = extract_oauth_code(stream).await?;
        code_tx.send(code).unwrap();

        Ok::<(), anyhow::Error>(())
    });

    // Step 4: Wait for code
    let code = code_rx.await?;

    // Step 5: Exchange code for tokens
    let token_response = exchange_code_for_tokens(&code).await?;

    // Step 6: Store credentials via IPC
    ipc_client.request("credentials.store", json!({
        "integration_id": "finance",
        "credential_id": "gmail_oauth",
        "credential_type": "OAuth2",
        "data": {
            "access_token": token_response.access_token,
            "refresh_token": token_response.refresh_token,
            "scopes": ["https://www.googleapis.com/auth/gmail.readonly"],
            "expires_at": Utc::now() + Duration::seconds(token_response.expires_in)
        }
    })).await?;

    Ok(())
}
```

### 2. Email Scanning Job

```rust
pub struct EmailScanJob {
    provider: EmailProvider,
    last_uid: Option<String>,
    processed: usize,
    total: usize,
}

impl EmailScanJob {
    pub async fn run(&mut self, ipc: &mut IpcClient) -> Result<()> {
        // Get credentials
        let creds_resp = ipc.request("credentials.get", json!({
            "integration_id": "finance",
            "credential_id": "gmail_oauth"
        })).await?;

        let oauth_token = creds_resp.data
            .and_then(|d| d.get("access_token"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| anyhow::anyhow!("No access token"))?;

        // Connect to Gmail
        let gmail = GmailClient::new(oauth_token);

        // Search for receipts
        let query = "subject:(receipt OR invoice) has:attachment";
        let messages = gmail.search(query, self.last_uid.as_ref()).await?;

        self.total = messages.len();

        for msg in messages {
            // Process message
            self.process_message(&msg, ipc).await?;

            self.processed += 1;
            self.last_uid = Some(msg.id.clone());

            // Report progress via IPC
            ipc.request("job.report_progress", json!({
                "job_id": self.job_id(),
                "progress": self.processed as f32 / self.total as f32,
                "message": format!("Processed {}/{} messages", self.processed, self.total)
            })).await?;
        }

        Ok(())
    }

    async fn process_message(&self, msg: &GmailMessage, ipc: &mut IpcClient) -> Result<()> {
        // Download full message
        let email_raw = msg.get_raw().await?;

        // Parse email
        let parsed = mail_parser::MessageParser::default().parse(&email_raw)?;

        // Check if it's a receipt (heuristic)
        if !self.is_receipt(&parsed) {
            return Ok(());
        }

        // Extract vendor and date from subject/body
        let metadata = self.extract_metadata(&parsed);

        // Create VDFS entry via IPC
        let entry_resp = ipc.request("vdfs.create_entry", json!({
            "name": format!("Receipt: {} - {}", metadata.vendor, metadata.date),
            "path": format!("extensions/finance/receipts/{}.eml", msg.id),
            "entry_type": "FinancialDocument"
        })).await?;

        let entry_id: Uuid = serde_json::from_value(
            entry_resp.data.unwrap()["entry_id"].clone()
        )?;

        // Store email sidecar
        ipc.request("vdfs.write_sidecar", json!({
            "entry_id": entry_id,
            "filename": "email.json",
            "data": base64::encode(serde_json::to_vec(&parsed)?)
        })).await?;

        // Process attachments
        for attachment in parsed.attachments {
            if attachment.is_pdf() || attachment.is_image() {
                // Queue OCR job
                ipc.request("jobs.dispatch", json!({
                    "job_type": "ocr",
                    "params": {
                        "entry_id": entry_id,
                        "attachment_data": base64::encode(&attachment.data)
                    }
                })).await?;
            }
        }

        Ok(())
    }

    fn is_receipt(&self, email: &ParsedEmail) -> bool {
        // Heuristic detection
        let subject_lower = email.subject.to_lowercase();
        let body_lower = email.body_text.to_lowercase();

        let receipt_keywords = [
            "receipt", "invoice", "payment", "order confirmation",
            "purchase", "transaction", "paid"
        ];

        receipt_keywords.iter().any(|kw| {
            subject_lower.contains(kw) || body_lower.contains(kw)
        })
    }
}
```

### 3. OCR Processing

```rust
pub async fn process_ocr(entry_id: Uuid, pdf_data: Vec<u8>, ipc: &mut IpcClient) -> Result<()> {
    // Call Spacedrive's OCR service via IPC
    let ocr_resp = ipc.request("ai.ocr", json!({
        "data": base64::encode(&pdf_data),
        "options": {
            "language": "eng",
            "preprocessing": true,
            "engine": "tesseract"
        }
    })).await?;

    let ocr_text: String = serde_json::from_value(
        ocr_resp.data.unwrap()["text"].clone()
    )?;

    // Store OCR result in sidecar
    ipc.request("vdfs.write_sidecar", json!({
        "entry_id": entry_id,
        "filename": "ocr.txt",
        "data": base64::encode(ocr_text.as_bytes())
    })).await?;

    // Trigger classification
    classify_receipt(entry_id, &ocr_text, ipc).await?;

    Ok(())
}
```

### 4. AI Classification

```rust
pub async fn classify_receipt(
    entry_id: Uuid,
    ocr_text: &str,
    ipc: &mut IpcClient
) -> Result<()> {
    let prompt = format!(r#"
Extract structured data from this receipt. Return JSON only.

Receipt Text:
{}

Required fields:
- vendor: Company name
- amount: Total amount (number only)
- currency: 3-letter code (USD, EUR, etc.)
- date: ISO 8601 format
- category: One of [Food & Dining, Transportation, Office Supplies, Travel, Entertainment, Other]
- items: Array of {{name, price}}
- tax: Tax amount (number only)

JSON:
"#, ocr_text);

    // Call AI service via IPC
    let ai_resp = ipc.request("ai.complete", json!({
        "prompt": prompt,
        "options": {
            "model": "user_default", // Respects user's AI settings
            "temperature": 0.1,
            "max_tokens": 500
        }
    })).await?;

    let response_text: String = serde_json::from_value(
        ai_resp.data.unwrap()["text"].clone()
    )?;

    // Parse JSON response
    let receipt_data: ReceiptData = serde_json::from_str(&response_text)?;

    // Store analysis in sidecar
    ipc.request("vdfs.write_sidecar", json!({
        "entry_id": entry_id,
        "filename": "receipt_analysis.json",
        "data": base64::encode(serde_json::to_vec(&receipt_data)?)
    })).await?;

    // Update entry metadata for search
    ipc.request("vdfs.update_metadata", json!({
        "entry_id": entry_id,
        "metadata": {
            "vendor": receipt_data.vendor,
            "amount": receipt_data.amount,
            "category": receipt_data.category,
            "date": receipt_data.date
        }
    })).await?;

    Ok(())
}
```

---

## Data Model

### Entry Structure

```rust
pub struct ReceiptEntry {
    // VDFS Entry fields
    pub id: Uuid,
    pub path: SdPath,
    pub name: String,
    pub entry_type: EntryType::FinancialDocument,

    // Custom metadata
    pub vendor: String,
    pub amount: f64,
    pub currency: String,
    pub date: NaiveDate,
    pub category: ExpenseCategory,
}
```

### Sidecar Files

**`email.json`** - Raw email metadata
```json
{
  "from": "receipts@starbucks.com",
  "to": "user@example.com",
  "subject": "Your Starbucks Receipt",
  "date": "2025-01-15T10:23:00Z",
  "message_id": "<abc123@starbucks.com>",
  "body_text": "Thank you for your purchase...",
  "body_html": "<html>...</html>",
  "attachments": [
    {
      "filename": "receipt.pdf",
      "content_type": "application/pdf",
      "size": 52341
    }
  ]
}
```

**`ocr.txt`** - Extracted text
```
STARBUCKS COFFEE COMPANY
Store #12345
123 Main St, San Francisco CA

Date: 01/15/2025 10:23 AM

Caffe Latte          $5.95
Croissant            $2.52
                    ------
Subtotal             $8.47
Tax                  $0.68
                    ------
Total                $9.15

Payment: Visa ****4532
```

**`receipt_analysis.json`** - AI-extracted data
```json
{
  "vendor": "Starbucks Coffee Company",
  "amount": 9.15,
  "currency": "USD",
  "date": "2025-01-15",
  "category": "Food & Dining",
  "items": [
    { "name": "Caffe Latte", "price": 5.95 },
    { "name": "Croissant", "price": 2.52 }
  ],
  "tax": 0.68,
  "payment_method": "Visa ****4532",
  "location": "Store #12345, 123 Main St, San Francisco CA",
  "confidence": 0.96,
  "extracted_at": "2025-01-15T10:30:00Z"
}
```

---

## Receipt Processing Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  1. Email Scanning (EmailScanJob)                               │
│                                                                  │
│  • Connect to Gmail/Outlook/IMAP                                │
│  • Search: "subject:(receipt OR invoice) has:attachment"        │
│  • Filter by last processed UID                                 │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  2. Receipt Detection (Heuristic)                               │
│                                                                  │
│  • Check subject/body for keywords                              │
│  • Look for attachments (PDF, image)                            │
│  • Extract sender domain patterns                               │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  3. Entry Creation (via IPC)                                    │
│                                                                  │
│  • Create VDFS Entry                                            │
│  • Store email.json sidecar                                     │
│  • Save attachment data                                         │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  4. OCR Processing (OcrJob)                                     │
│                                                                  │
│  • Call ai.ocr() via IPC                                        │
│  • Extract text from PDF/image                                  │
│  • Store ocr.txt sidecar                                        │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  5. AI Classification (ai.complete via IPC)                     │
│                                                                  │
│  • Structured prompt with OCR text                              │
│  • Parse JSON response                                          │
│  • Store receipt_analysis.json sidecar                          │
│  • Update entry metadata                                        │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│  6. Search Indexing (Automatic via Event Bus)                  │
│                                                                  │
│  • Entry created event → search service                         │
│  • Index vendor, amount, date, category                         │
│  • Generate embedding for semantic search                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## API Specification

### Core → Extension APIs (What Extension Can Call)

**VDFS Operations:**
```typescript
// Create new entry
interface CreateEntryRequest {
  name: string;
  path: string;
  entry_type: "FinancialDocument" | "Email" | "Receipt";
  metadata?: Record<string, any>;
}

// Write sidecar file
interface WriteSidecarRequest {
  entry_id: string;
  filename: string;
  data: Uint8Array; // base64 encoded in JSON
}

// Update entry metadata
interface UpdateMetadataRequest {
  entry_id: string;
  metadata: Record<string, any>;
}
```

**Job System:**
```typescript
// Dispatch job
interface DispatchJobRequest {
  job_type: "email_scan" | "ocr" | "classification";
  params: Record<string, any>;
  resumable?: boolean;
}

// Report progress
interface ReportProgressRequest {
  job_id: string;
  progress: number; // 0.0 to 1.0
  message?: string;
}
```

**AI Service:**
```typescript
// OCR
interface OcrRequest {
  data: Uint8Array; // PDF or image
  options: {
    language: string;
    preprocessing?: boolean;
    engine: "tesseract" | "easyocr";
  };
}

// Classification
interface CompleteRequest {
  prompt: string;
  options: {
    model: "user_default" | string;
    temperature: number;
    max_tokens: number;
  };
}
```

**Credential Manager:**
```typescript
// Store credential
interface StoreCredentialRequest {
  integration_id: string;
  credential_id: string;
  credential_type: "OAuth2" | "ApiKey" | "Basic";
  data: {
    access_token?: string;
    refresh_token?: string;
    api_key?: string;
    username?: string;
    password?: string;
  };
}

// Get credential (auto-refreshes OAuth2)
interface GetCredentialRequest {
  integration_id: string;
  credential_id: string;
}
```

### Extension → Core Events (What Extension Can Subscribe To)

```typescript
// Entry created
interface EntryCreatedEvent {
  entry_id: string;
  path: string;
  entry_type: string;
}

// Entry modified
interface EntryModifiedEvent {
  entry_id: string;
  old_metadata: Record<string, any>;
  new_metadata: Record<string, any>;
}

// Job status change
interface JobStatusEvent {
  job_id: string;
  status: "queued" | "running" | "completed" | "failed";
  progress: number;
}
```

---

## Prototype Implementation

### Week 1: Foundation (40 hours)

**Day 1-2: Project Setup**
- [ ] Create `spacedrive-finance` Rust project
- [ ] Set up IPC client library
- [ ] Implement basic IPC protocol
- [ ] Test connection to Spacedrive core

**Day 3-4: Gmail OAuth**
- [ ] Implement OAuth flow (authorization URL + callback server)
- [ ] Exchange code for tokens
- [ ] Store credentials via IPC
- [ ] Test token refresh

**Day 5: Email Scanning Basics**
- [ ] Gmail API client
- [ ] Search for receipts (keyword-based)
- [ ] Download message metadata
- [ ] Parse email structure

### Week 2: Core Processing (40 hours)

**Day 1-2: Entry Creation**
- [ ] Create VDFS entries via IPC
- [ ] Store email.json sidecars
- [ ] Handle attachments (download + store)
- [ ] Test with sample emails

**Day 3: OCR Integration**
- [ ] Call ai.ocr() via IPC
- [ ] Process PDF attachments
- [ ] Store ocr.txt sidecars
- [ ] Error handling

**Day 4-5: AI Classification**
- [ ] Design classification prompt
- [ ] Call ai.complete() via IPC
- [ ] Parse JSON responses
- [ ] Store receipt_analysis.json
- [ ] Update entry metadata

### Week 3: Polish & Testing (40 hours)

**Day 1-2: Job System**
- [ ] Wrap scanning in resumable job
- [ ] Progress reporting
- [ ] Error handling and retries
- [ ] Test job resumption

**Day 3: UI Integration**
- [ ] Settings panel (connect email)
- [ ] Receipt list view
- [ ] Export to CSV
- [ ] Search integration

**Day 4-5: Testing**
- [ ] Integration tests with real Gmail
- [ ] Test with various receipt formats
- [ ] Performance testing (1000+ receipts)
- [ ] Bug fixes

### Deliverable

**Functional MVP:**
- Connect to Gmail via OAuth
- Scan inbox for receipts
- Extract text via OCR
- Classify with AI
- Searchable in Spacedrive
- Export to CSV

**Not Included (v2):**
- Outlook/IMAP support (Gmail only)
- Multi-currency
- QuickBooks API integration
- Mobile scanning
- Automatic vendor reconciliation

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_detection() {
        let email = ParsedEmail {
            subject: "Your Starbucks Receipt".to_string(),
            body_text: "Thank you for your purchase".to_string(),
            ..Default::default()
        };

        assert!(is_receipt(&email));
    }

    #[test]
    fn test_metadata_extraction() {
        let ocr_text = r#"
            STARBUCKS COFFEE COMPANY
            Date: 01/15/2025
            Total: $9.15
        "#;

        let metadata = extract_metadata(ocr_text);
        assert_eq!(metadata.vendor, "Starbucks Coffee Company");
        assert_eq!(metadata.amount, 9.15);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_receipt_pipeline() {
    let mut ipc = IpcClient::connect().await.unwrap();

    // Load test receipt email
    let email_bytes = include_bytes!("fixtures/starbucks_receipt.eml");

    // Process
    let entry_id = process_receipt_email(email_bytes, &mut ipc).await.unwrap();

    // Verify entry created
    let entry = ipc.request("vdfs.get_entry", json!({
        "entry_id": entry_id
    })).await.unwrap();

    assert!(entry.success);

    // Verify sidecar exists
    let sidecar = ipc.request("vdfs.read_sidecar", json!({
        "entry_id": entry_id,
        "filename": "receipt_analysis.json"
    })).await.unwrap();

    let receipt_data: ReceiptData = serde_json::from_value(
        sidecar.data.unwrap()
    ).unwrap();

    assert_eq!(receipt_data.vendor, "Starbucks Coffee Company");
    assert_eq!(receipt_data.amount, 9.15);
}
```

### Performance Benchmarks

```rust
#[tokio::test]
async fn bench_receipt_processing() {
    let start = Instant::now();

    // Process 100 receipts
    for i in 0..100 {
        let email = generate_test_email(i);
        process_receipt_email(&email, &mut ipc).await.unwrap();
    }

    let duration = start.elapsed();
    let per_receipt = duration / 100;

    // Should process at least 1 receipt/second
    assert!(per_receipt < Duration::from_secs(1));
}
```

---

## Phase 2: WASM Migration (Future)

Once the WASM plugin system is built, migration path:

### 1. Extract Core Logic

Move business logic to shared library:

```rust
// spacedrive-finance-core/src/lib.rs
pub mod email;
pub mod receipt;

// Shared between process-based and WASM versions
pub async fn process_receipt(
    email_data: &[u8],
    api: &dyn SpacedriveApi // Trait abstraction
) -> Result<ReceiptData> {
    let parsed = parse_email(email_data)?;
    let is_receipt = detect_receipt(&parsed);

    if !is_receipt {
        return Ok(None);
    }

    let entry_id = api.create_entry(...).await?;
    let ocr_text = api.ocr(...).await?;
    let receipt_data = api.classify(...).await?;

    Ok(receipt_data)
}
```

### 2. WASM Wrapper

```rust
// spacedrive-finance-wasm/src/lib.rs
use spacedrive_finance_core::*;

#[spacedrive_plugin]
pub struct FinancePlugin {
    core: FinanceCore,
}

impl SpacedrivePlugin for FinancePlugin {
    fn init(&mut self, ctx: &PluginContext) -> Result<()> {
        self.core = FinanceCore::new(ctx);
        Ok(())
    }

    fn on_entry_created(&mut self, entry: &Entry) -> Result<Vec<Action>> {
        if entry.is_email() {
            // Process via shared core logic
            let receipt = process_receipt(entry.data(), self).await?;
            Ok(vec![Action::ClassifyReceipt(receipt)])
        } else {
            Ok(vec![])
        }
    }
}

// Implement SpacedriveApi trait for WASM environment
impl SpacedriveApi for FinancePlugin {
    async fn create_entry(&self, ...) -> Result<Uuid> {
        // Direct WASM host function call
        unsafe {
            vdfs_create_entry(...)
        }
    }

    async fn ocr(&self, data: &[u8]) -> Result<String> {
        unsafe {
            ai_ocr(...)
        }
    }
}
```

### 3. Build & Distribution

```bash
# Compile to WASM
cargo build --target wasm32-unknown-unknown --release

# Package
cp target/wasm32-unknown-unknown/release/spacedrive_finance.wasm dist/
cp manifest.json dist/
tar -czf spacedrive-finance-v1.0.0.wasm.tar.gz dist/

# Upload to marketplace
spacedrive plugin publish spacedrive-finance-v1.0.0.wasm.tar.gz
```

---

## Summary

This design provides a **concrete path from concept to revenue**:

1. **Week 1-3:** Ship process-based MVP (fast iteration)
2. **Validate:** 100 paying users = proof of revenue model
3. **Build Platform:** WASM system developed in parallel
4. **Migrate:** Refactor to WASM once platform exists

**Key Advantages:**
- Start generating revenue in weeks, not months
- Learn from real users before committing to WASM
- Validate integration points with actual usage
- Smooth migration path (shared core logic)

**Next Step:** Start coding `spacedrive-finance` prototype!

