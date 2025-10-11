<!--CREATED: 2025-10-09-->
# Extension SDK API Vision - The Sexiest API

**Goal:** Extension development should feel like magic. Zero boilerplate, maximum clarity.

---

## Current API (Functional but Rough)

### Defining a Job

```rust
#[derive(Serialize, Deserialize)]
pub struct EmailScanState {
    last_uid: String,
    processed: usize,
}

#[no_mangle]
pub extern "C" fn execute_email_scan(
    ctx_json_ptr: u32,
    ctx_json_len: u32,
    state_json_ptr: u32,
    state_json_len: u32
) -> i32 {
    let ctx_json = unsafe {
        let slice = std::slice::from_raw_parts(ctx_json_ptr as *const u8, ctx_json_len as usize);
        std::str::from_utf8(slice).unwrap_or("{}")
    };

    let job_ctx = JobContext::from_params(ctx_json).unwrap();
    let mut state: EmailScanState = if state_json_len > 0 {
        // ... manual deserialization
    } else {
        // ... initialization
    };

    // ... job logic ...

    JobResult::Completed.to_exit_code()
}
```

**Problems:**
- Manual `#[no_mangle]` and `extern "C"`
- Ugly pointer/length parameters
- Manual serialization/deserialization
- Returns i32 instead of Result
- Boilerplate everywhere

---

## SEXY API v1: Attribute Macros

### Defining a Job

```rust
use spacedrive_sdk::prelude::*;

#[derive(Serialize, Deserialize, Default)]
pub struct EmailScanState {
    last_uid: String,
    processed: usize,
}

#[job]
async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
    ctx.log(&format!("Scanning from UID: {}", state.last_uid));

    let emails = fetch_emails(&state.last_uid)?;

    for (i, email) in emails.iter().enumerate() {
        // Check interruption - macro handles checkpoint!
        ctx.check_interrupt()?;

        // Process email
        process_email(ctx, email).await?;
        state.last_uid = email.uid.clone();
        state.processed += 1;

        // Report progress - macro handles!
        ctx.progress((i + 1) as f32 / emails.len() as f32);
    }

    Ok(())
}
```

**What `#[job]` generates:**
- `#[no_mangle] pub extern "C" fn execute_email_scan(...) -> i32`
- Parameter marshalling (pointers → types)
- State load/save logic
- Error handling (? → JobResult::Failed)
- Auto-checkpoint on `check_interrupt()?`
- Progress tracking
- Return code conversion

**Developer writes:** 20 lines of business logic
**Macro generates:** 50+ lines of boilerplate

### Defining a Query/Action

```rust
#[spacedrive_query]
async fn classify_receipt(ctx: &ExtensionContext, pdf_data: Vec<u8>) -> Result<ReceiptData> {
    // Just write the logic!
    let ocr = ctx.ai().ocr(&pdf_data, OcrOptions::default())?;
    let analysis = ctx.ai().classify_text(&ocr.text, "Extract receipt data")?;

    Ok(ReceiptData {
        vendor: analysis["vendor"].as_str().unwrap().into(),
        amount: analysis["amount"].as_f64().unwrap(),
        date: analysis["date"].as_str().unwrap().into(),
    })
}
```

**What `#[spacedrive_query]` generates:**
- Wire method registration (`query:finance:classify_receipt.v1`)
- FFI export function
- Input/output serialization
- Error handling
- Automatic registration in `plugin_init()`

---

## SEXY API v2: Declarative Extension Definition

```rust
use spacedrive_sdk::prelude::*;

#[spacedrive_extension(
    id = "finance",
    name = "Spacedrive Finance",
    version = "0.1.0"
)]
mod finance_extension {
    use super::*;

    // === Jobs ===

    #[job(resumable = true)]
    async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
        for email in fetch_emails(&state.last_uid)? {
            ctx.check_interrupt()?;  // Auto-checkpoints!

            let entry = ctx.vdfs().create_entry(CreateEntry {
                name: format!("Receipt: {}", email.subject),
                ..Default::default()
            })?;

            state.last_uid = email.uid;
            ctx.progress_auto();  // Auto-calculates from iterator!
        }
        Ok(())
    }

    // === Queries ===

    #[query]
    async fn classify_receipt(ctx: &ExtensionContext, pdf: Vec<u8>) -> Result<ReceiptData> {
        let ocr = ctx.ai().ocr(&pdf, OcrOptions::default())?;
        parse_receipt(&ocr.text)
    }

    #[query]
    async fn search_receipts(
        ctx: &ExtensionContext,
        #[param(default = "last_month")] date_range: DateRange,
        #[param(optional)] vendor: Option<String>
    ) -> Result<Vec<Receipt>> {
        // Query logic
        todo!()
    }

    // === Actions ===

    #[action]
    async fn import_receipts(
        ctx: &ExtensionContext,
        emails: Vec<Email>
    ) -> Result<ImportResult> {
        let mut imported = vec![];

        for email in emails {
            let entry = ctx.vdfs().create_entry(CreateEntry {
                name: format!("Receipt: {}", email.subject),
                ..Default::default()
            })?;
            imported.push(entry.id);
        }

        Ok(ImportResult { imported_count: imported.len() })
    }

    // === Event Handlers ===

    #[on_entry_created(filter = "entry.entry_type == 'Email'")]
    async fn on_email_received(ctx: &ExtensionContext, entry: Entry) {
        // Automatically triggered when email entries are created!
        if is_receipt(&entry) {
            ctx.log("Receipt detected, queueing analysis...");
            ctx.dispatch_job("finance:classify_receipt", json!({ "entry_id": entry.id })).ok();
        }
    }

    // === Configuration ===

    #[config]
    struct FinanceConfig {
        #[config(default = "gmail")]
        email_provider: String,

        #[config(secret)]
        api_key: Option<String>,

        #[config(default = vec!["Food & Dining", "Travel"])]
        categories: Vec<String>,
    }
}
```

**What this generates:**
- All Wire method registrations
- All FFI exports
- Automatic `plugin_init()` that registers everything
- Event subscription setup
- Config validation and loading
- Type-safe builders for all inputs

**Developer writes:** Pure business logic
**Macro generates:** All infrastructure

---

## SEXY API v3: Builder Pattern + Fluent API

### Job Execution

```rust
#[job]
async fn process_receipts(ctx: &JobContext, state: &mut ProcessState) -> Result<()> {
    // Fluent progress reporting
    ctx.with_progress("Fetching emails...")
        .items(state.emails.len())
        .for_each(&state.emails, |email| async {
            process_email(ctx, email).await
        })
        .await?;

    // Builder-style operations
    ctx.vdfs()
        .create_entry("Receipt: Starbucks")
        .at_path("receipts/1.eml")
        .with_type("FinancialDocument")
        .with_metadata(json!({ "vendor": "Starbucks" }))
        .execute()?;

    Ok(())
}
```

### Chaining Operations

```rust
#[spacedrive_query]
async fn analyze_receipt(ctx: &ExtensionContext, pdf: Vec<u8>) -> Result<ReceiptData> {
    ctx.ai()
        .ocr(&pdf)
        .with_language("eng")
        .with_preprocessing()
        .execute()?
        .then(|ocr| {
            ctx.ai()
                .classify(&ocr.text)
                .with_prompt("Extract vendor, amount, date")
                .with_temperature(0.1)
                .execute()
        })?
        .then(|analysis| {
            ReceiptData::from_json(analysis)
        })
}
```

---

## SEXY API v4: Derive Macros

### Auto-Implement Common Patterns

```rust
#[derive(SpacedriveEntry)]
#[entry_type = "FinancialDocument"]
struct Receipt {
    id: Uuid,

    #[sidecar]
    email_data: EmailMetadata,

    #[sidecar]
    ocr_text: String,

    #[sidecar]
    analysis: ReceiptAnalysis,

    #[metadata]
    vendor: String,

    #[metadata]
    amount: f64,
}

impl Receipt {
    // Auto-generated methods:
    // - save() - creates entry + sidecars
    // - load(id) - loads entry + sidecars
    // - update() - updates metadata
    // - delete() - removes entry + sidecars
}

// Usage:
let receipt = Receipt {
    email_data: email_metadata,
    ocr_text: ocr_result.text,
    analysis: ai_analysis,
    vendor: "Starbucks".into(),
    amount: 8.47,
    ..Default::default()
};

receipt.save(ctx)?;  // One call!
```

---

## SEXY API v5: Query DSL

```rust
#[spacedrive_query]
async fn search_receipts(ctx: &ExtensionContext, params: SearchParams) -> Result<Vec<Receipt>> {
    ctx.search()
        .entries()
        .of_type("FinancialDocument")
        .where_metadata(|m| {
            m.field("vendor").contains(params.vendor_query)
             .and()
             .field("amount").greater_than(params.min_amount)
             .and()
             .field("date").in_range(params.start_date, params.end_date)
        })
        .order_by("date", Desc)
        .limit(100)
        .execute()
        .await?
        .map(|entry| Receipt::from_entry(entry))
        .collect()
}
```

---

## SEXY API v6: The Ultimate - Minimal Boilerplate

```rust
use spacedrive_sdk::prelude::*;

// === Extension Definition ===

#[extension(
    id = "finance",
    name = "Spacedrive Finance",
    version = "0.1.0"
)]
struct FinanceExtension;

// === Jobs (Resumable, Progress-Tracked) ===

#[job]
impl FinanceExtension {
    async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
        for email in fetch_emails(&state.last_uid)?.progress(ctx) {
            ctx.check()?;  // Auto-checkpoints!
            process_email(ctx, email).await?;
            state.last_uid = email.uid;
        }
        Ok(())
    }
}

// === Queries (Read-Only) ===

#[query]
impl FinanceExtension {
    async fn classify_receipt(pdf: Vec<u8>, ctx: &AI) -> Result<ReceiptData> {
        let ocr = ctx.ocr(&pdf).await?;
        ctx.classify(&ocr.text, "Extract receipt data").await
    }

    async fn search_receipts(
        vendor: Option<String>,
        min_amount: f64,
        ctx: &Search
    ) -> Result<Vec<Receipt>> {
        ctx.find::<Receipt>()
            .vendor(vendor)
            .min_amount(min_amount)
            .execute()
            .await
    }
}

// === Actions (State-Changing) ===

#[action]
impl FinanceExtension {
    async fn import_from_email(
        provider: EmailProvider,
        ctx: &VDFS
    ) -> Result<ImportResult> {
        let emails = fetch_emails(provider).await?;

        emails.par_iter()
            .map(|email| ctx.create_entry(email.into()))
            .collect()
    }
}

// === Event Handlers ===

#[on_event(EntryCreated, filter = "entry_type == 'Email'")]
impl FinanceExtension {
    async fn on_email_created(entry: Entry, ctx: &ExtensionContext) {
        if is_receipt(&entry) {
            ctx.dispatch("finance:classify_receipt", entry.id).await.ok();
        }
    }
}

// === Configuration ===

#[config]
struct FinanceConfig {
    #[default = "gmail"]
    email_provider: String,

    #[secret]
    oauth_token: Option<String>,
}
```

**That's an ENTIRE extension in ~60 lines!**

---

## Macro Implementations

### 1. `#[job]` - The Job Macro

**Usage:**
```rust
#[job(resumable = true, name = "email_scan")]
async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
    // Just write business logic!
    for email in fetch_emails(&state.last_uid)? {
        ctx.check()?;  // Returns Err on interrupt
        process_email(ctx, email).await?;
        state.last_uid = email.uid;
    }
    Ok(())
}
```

**Generates:**
```rust
#[no_mangle]
pub extern "C" fn execute_email_scan(
    ctx_ptr: u32,
    ctx_len: u32,
    state_ptr: u32,
    state_len: u32
) -> i32 {
    // Generated boilerplate:
    let ctx_json = read_string_from_ptr(ctx_ptr, ctx_len);
    let job_ctx = JobContext::from_params(&ctx_json).unwrap();

    let mut state: EmailScanState = if state_len > 0 {
        deserialize_state(state_ptr, state_len).unwrap()
    } else {
        EmailScanState::default()
    };

    // Call user's function
    let result = tokio::runtime::Handle::current().block_on(async {
        email_scan(&job_ctx, &mut state).await
    });

    // Handle result
    match result {
        Ok(_) => {
            job_ctx.log("Job completed");
            JobResult::Completed.to_exit_code()
        }
        Err(e) if e.is_interrupt() => {
            job_ctx.checkpoint(&state).ok();
            JobResult::Interrupted.to_exit_code()
        }
        Err(e) => {
            job_ctx.log_error(&e.to_string());
            JobResult::Failed(e.to_string()).to_exit_code()
        }
    }
}

// Also generates registration in plugin_init()
```

### 2. `#[spacedrive_query]` - Query Macro

**Usage:**
```rust
#[spacedrive_query]
async fn classify_receipt(
    ctx: &ExtensionContext,
    pdf_data: Vec<u8>,
    #[param(default = "eng")] language: String
) -> Result<ReceiptData> {
    let ocr = ctx.ai().ocr(&pdf_data, OcrOptions {
        language,
        ..Default::default()
    })?;

    parse_receipt(&ocr.text)
}
```

**Generates:**
```rust
// Wire method: "query:finance:classify_receipt.v1"

#[derive(Serialize, Deserialize)]
struct ClassifyReceiptInput {
    pdf_data: Vec<u8>,
    #[serde(default = "default_language")]
    language: String,
}

#[no_mangle]
pub extern "C" fn handle_classify_receipt(input_ptr: u32, input_len: u32) -> u32 {
    let input: ClassifyReceiptInput = deserialize_input(input_ptr, input_len).unwrap();
    let ctx = ExtensionContext::new(get_library_id());

    let result = tokio::runtime::Handle::current().block_on(async {
        classify_receipt(&ctx, input.pdf_data, input.language).await
    });

    match result {
        Ok(data) => serialize_output(&data),
        Err(e) => serialize_error(&e),
    }
}

// Registration in plugin_init()
```

### 3. `#[extension]` - Extension Container Macro

**Usage:**
```rust
#[extension(
    id = "finance",
    name = "Spacedrive Finance",
    permissions = ["vdfs.*", "ai.*", "credentials.*"]
)]
struct FinanceExtension {
    config: FinanceConfig,
}

#[extension_impl]
impl FinanceExtension {
    // Automatically becomes plugin_init()
    fn init(&mut self) -> Result<()> {
        self.log("Finance extension starting...");
        self.config.load()?;
        Ok(())
    }

    // All methods become operations based on attributes

    #[job]
    async fn email_scan(&self, ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
        // Job logic
    }

    #[query]
    async fn classify_receipt(&self, pdf: Vec<u8>) -> Result<ReceiptData> {
        // Query logic
    }
}
```

**Generates:**
- `manifest.json`
- All FFI exports
- Registration code
- `self` context available in all methods

### 4. Ergonomic Error Handling

**Custom `?` operator:**
```rust
#[job]
async fn scan_emails(ctx: &JobContext, state: &mut State) -> Result<()> {
    let emails = fetch_emails(&state.last_uid)?;
    //                                         ^ On error:
    //                                           - Logs error
    //                                           - Saves checkpoint
    //                                           - Returns Failed

    for email in emails {
        ctx.check()?;  // On interrupt:
        //           - Saves checkpoint
        //           - Returns Interrupted

        process_email(ctx, email).await?;
    }

    Ok(())
}
```

### 5. Progress Helpers

```rust
#[job]
async fn process_batch(ctx: &JobContext, state: &mut State) -> Result<()> {
    // Auto-progress from iterator!
    for item in ctx.progress_iter(&items, "Processing items") {
        process_item(item)?;
        // Progress automatically reported!
        // Checkpoints automatically saved every 10!
    }

    // Or manual with helpers
    ctx.progress().at(0.5).message("Halfway done").report();

    // Or super simple
    ctx.progress_auto();  // Infers from context

    Ok(())
}
```

### 6. Type-Safe Entry Operations

```rust
#[derive(SpacedriveEntry)]
#[entry_type = "FinancialDocument"]
struct Receipt {
    #[entry_field]
    id: Uuid,

    #[metadata]
    vendor: String,

    #[metadata]
    amount: f64,

    #[sidecar(file = "email.json")]
    email: EmailData,

    #[sidecar(file = "ocr.txt")]
    ocr_text: String,

    #[sidecar(file = "analysis.json")]
    analysis: ReceiptAnalysis,
}

// Usage:
let receipt = Receipt::new(ctx)
    .vendor("Starbucks")
    .amount(8.47)
    .with_sidecar_email(email_data)
    .with_sidecar_ocr(ocr_text)
    .with_sidecar_analysis(analysis)
    .save()?;

// Later:
let receipt = Receipt::load(ctx, receipt_id)?;
receipt.analysis.category = "Food & Dining";
receipt.update()?;

// Search:
let receipts = Receipt::search(ctx)
    .vendor("Starbucks")
    .amount_greater_than(5.0)
    .in_date_range(start, end)
    .execute()?;
```

---

## The Absolute Sexiest: Natural Language DSL

### Conceptual (Probably Too Far)

```rust
#[extension = "finance"]

job email_scan(state: EmailScanState) {
    fetch emails where uid > state.last_uid

    for each email:
        create entry from email
        run ocr on email.attachment
        classify ocr.text as receipt_data
        save to entry.sidecars

        progress += 1
        checkpoint if progress % 10 == 0
}

query classify_receipt(pdf: Vec<u8>) -> ReceiptData {
    ocr_text = ai.ocr(pdf, language = "eng")
    analysis = ai.classify(ocr_text, prompt = "Extract receipt fields")
    return ReceiptData.from_json(analysis)
}

on entry_created where entry_type == "Email" {
    if is_receipt(entry):
        dispatch classify_receipt(entry.id)
}
```

---

## Recommended Implementation

### Phase 1: Core Macros (Week 1)

**Priority Order:**

1. **`#[job]`** - Biggest pain point
   - Eliminates all FFI boilerplate
   - Auto-handles state save/load
   - Progress and checkpoint helpers

2. **`#[spacedrive_query]` + `#[spacedrive_action]`** - Second priority
   - Auto-generates FFI exports
   - Handles serialization
   - Wire registration

3. **`#[extension]`** - Container macro
   - Generates `plugin_init()` and `plugin_cleanup()`
   - Auto-registers all operations
   - Config management

### Phase 2: Ergonomic Helpers (Week 2)

4. **`#[derive(SpacedriveEntry)]`** - Type-safe entries
   - Auto-sidecar management
   - Builder patterns
   - Search helpers

5. **Progress helpers** - Iterator extensions
   - `ctx.progress_iter()`
   - Auto-checkpoint intervals
   - Fluent builders

---

## Example: Finance Extension with Sexy API

```rust
use spacedrive_sdk::prelude::*;

#[extension(id = "finance", name = "Spacedrive Finance")]
struct Finance {
    #[config]
    provider: EmailProvider,
}

#[extension_jobs]
impl Finance {
    #[job(resumable)]
    async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
        ctx.progress_iter(fetch_emails(&state.last_uid)?, "Scanning emails")
            .checkpoint_every(10)
            .for_each_async(|email| async {
                let entry = Receipt::from_email(email)
                    .run_ocr(ctx.ai())
                    .classify(ctx.ai())
                    .save(ctx.vdfs())?;

                state.last_uid = email.uid;
                Ok(())
            })
            .await
    }
}

#[extension_queries]
impl Finance {
    async fn search_receipts(
        vendor: Option<String>,
        date_range: DateRange,
        ctx: &Search
    ) -> Result<Vec<Receipt>> {
        Receipt::search(ctx)
            .vendor_like(vendor)
            .in_range(date_range)
            .execute()
            .await
    }
}

#[extension_events]
impl Finance {
    #[on(EntryCreated, filter = "entry_type == 'Email'")]
    async fn detect_receipt(entry: Entry, ctx: &ExtensionContext) {
        if is_receipt(&entry) {
            ctx.dispatch("finance:classify_receipt", entry.id).await.ok();
        }
    }
}
```

**30 lines of code. Full extension. Zero boilerplate. Pure magic. **

---

## Implementation Priority

### Must-Have (Phase 1):
- `#[job]` - 80% of developer pain
- `#[spacedrive_query]` / `#[spacedrive_action]` - Wire integration
- `#[extension]` - Container and registration

### Nice-to-Have (Phase 2):
- `#[derive(SpacedriveEntry)]` - Entry helpers
- Progress iterators
- Fluent builders

### Future:
- Event handler macros
- Natural language DSL (probably too far)

---

## Example Extension Before/After

### BEFORE (Current):

```rust
// 150+ lines of boilerplate
#[no_mangle]
pub extern "C" fn execute_email_scan(
    ctx_ptr: u32, ctx_len: u32,
    state_ptr: u32, state_len: u32
) -> i32 {
    let ctx_json = unsafe { /* ... */ };
    let job_ctx = JobContext::from_params(&ctx_json).unwrap();
    let mut state: EmailScanState = /* ... deserialization ... */;

    for email in fetch_emails(&state.last_uid).unwrap() {
        if job_ctx.check_interrupt() {
            job_ctx.checkpoint(&state).ok();
            return 1;
        }
        // ... logic ...
    }

    0
}
```

### AFTER (With Macros):

```rust
// 15 lines, zero boilerplate
#[job]
async fn email_scan(ctx: &JobContext, state: &mut EmailScanState) -> Result<()> {
    for email in fetch_emails(&state.last_uid)?.progress(ctx) {
        ctx.check()?;
        process_email(ctx, email).await?;
        state.last_uid = email.uid;
    }
    Ok(())
}
```

**90% less code. 100% more readable. Infinitely more maintainable.**

---

**Ready to build these macros and make extension development absolutely delightful?** 

