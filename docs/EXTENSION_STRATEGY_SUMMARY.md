# Spacedrive Extension Strategy: From Vision to Prototype

## Quick Navigation

📊 **Business Strategy** → [`docs/PLATFORM_REVENUE_MODEL.md`](./PLATFORM_REVENUE_MODEL.md)
🔧 **Technical Design** → [`docs/core/design/EMAIL_INGESTION_EXTENSION_DESIGN.md`](./core/design/EMAIL_INGESTION_EXTENSION_DESIGN.md)
📋 **Existing Architecture** → [`docs/core/design/INTEGRATION_SYSTEM_DESIGN.md`](./core/design/INTEGRATION_SYSTEM_DESIGN.md)
🎯 **WASM Tasks** → [`.tasks/PLUG-*.md`](../.tasks/)

---

## The Vision

**Spacedrive becomes a platform for local-first applications** that solve privacy-sensitive problems across multiple SaaS categories. Revenue comes from premium extensions, not cloud infrastructure.

### Why This Works

1. **Privacy Anxiety is Real** - WellyBox exists ($9.90-19.90/mo) but users hesitate: "Do I really want to give ANY third party full access to my financial documents?"

2. **Local AI is Here** - M-series chips, NPUs in consumer hardware, Ollama making local models practical

3. **Architecture Enables It** - Your v2 whitepaper architecture (VDFS, sidecars, AI layer, job system) provides the infrastructure that normally takes $10M+ to build

---

## The Two-Phase Strategy

### Phase 1: Process-Based MVP (NOW - Q1 2026)

**Goal:** Validate revenue with minimum engineering

**Approach:** Build `spacedrive-finance` as a **separate process** that talks to Spacedrive Core via IPC

**Why:**
- Ship in 2-3 weeks (vs. 3+ months for WASM platform)
- Use existing integration system (already designed)
- Validate willingness-to-pay before platform investment
- Learn what APIs extensions actually need

**Tech Stack:**
- Rust executable communicating over Unix sockets
- OAuth for Gmail/Outlook
- Calls core services via IPC: `vdfs.create_entry()`, `ai.ocr()`, `jobs.dispatch()`
- Standard OS-level process isolation

**Timeline:**
```
Week 1: Gmail OAuth + IPC protocol
Week 2: OCR + AI classification pipeline
Week 3: UI polish + testing
Launch: ProductHunt + HN + Reddit
```

### Phase 2: WASM Platform (Q3-Q4 2026)

**Goal:** Scalable third-party ecosystem

**Approach:** Build WebAssembly plugin system, migrate Finance extension

**Why:**
- Single `.wasm` file works everywhere (no platform-specific builds)
- True sandbox security (capability-based permissions)
- Hot-reload during development
- Enables marketplace with confidence

**Migration Path:**
1. Extract core logic to `spacedrive-finance-core` (Rust library)
2. Keep process-based wrapper for existing users
3. Add WASM wrapper using same core library
4. Gradual rollout to WASM version
5. Third-party devs use WASM from day one

---

## The Integration Points

The email extension leverages **7 core Spacedrive systems**:

| System | Purpose | API Call |
|--------|---------|----------|
| **VDFS** | Represent receipts as Entries | `vdfs.create_entry()` |
| **Sidecars** | Store email + AI analysis | `vdfs.write_sidecar()` |
| **Job System** | Durable email scanning | `jobs.dispatch()` |
| **AI Service** | OCR + classification | `ai.ocr()`, `ai.complete()` |
| **Credentials** | Secure OAuth tokens | `credentials.store()` |
| **Search** | Natural language queries | Auto via Event Bus |
| **Event Bus** | React to entry creation | `event_bus.subscribe()` |

### Example: Processing a Receipt

```rust
// 1. Scan Gmail for receipts
let messages = gmail.search("subject:(receipt OR invoice) has:attachment").await?;

// 2. Create Entry in VDFS
let entry_id = ipc.request("vdfs.create_entry", json!({
    "name": "Receipt: Starbucks - 2025-01-15",
    "entry_type": "FinancialDocument"
})).await?;

// 3. Store email data in sidecar
ipc.request("vdfs.write_sidecar", json!({
    "entry_id": entry_id,
    "filename": "email.json",
    "data": email_metadata
})).await?;

// 4. Extract text via OCR
let ocr_text = ipc.request("ai.ocr", json!({
    "data": pdf_attachment,
    "options": { "engine": "tesseract" }
})).await?;

// 5. Classify with AI (local or cloud)
let receipt = ipc.request("ai.complete", json!({
    "prompt": format!("Extract vendor, amount, date from: {}", ocr_text),
    "options": { "model": "user_default", "temperature": 0.1 }
})).await?;

// 6. Store analysis
ipc.request("vdfs.write_sidecar", json!({
    "entry_id": entry_id,
    "filename": "receipt_analysis.json",
    "data": receipt_data
})).await?;

// 7. Search indexes automatically via Event Bus
// User can now search: "coffee shops last quarter"
```

---

## What's Already Built vs. What We Need

### ✅ Already Exists (Ready to Use)

From the whitepaper and codebase:

- **VDFS Entry System** - Universal data model
- **Virtual Sidecars** - Structured data storage
- **Job System** - Durable background tasks
- **AI Layer** - OCR (Tesseract) + LLM integration (Ollama)
- **Search** - FTS + semantic embeddings
- **Credential Manager** - Encrypted storage (referenced in whitepaper)
- **Event Bus** - Loose coupling between services

### ❌ Needs Implementation (New Work)

**For Process-Based MVP:**
- [x] Integration Manager (IPC router, process lifecycle)
- [x] IPC protocol (JSON over Unix sockets)
- [x] Extension manifest format
- [x] OAuth flow helpers
- [x] Extension-specific APIs (wrap existing core services)

**For WASM Platform (Phase 2):**
- [ ] Wasmer/Wasmtime runtime integration (`.tasks/PLUG-001`)
- [ ] WASM Plugin Host with sandbox (`.tasks/PLUG-002`)
- [ ] VDFS API bridge (host functions)
- [ ] Permission system
- [ ] Plugin marketplace infrastructure

### 🔧 Integration Work Needed

**Core Services → IPC Exposure:**

Each core service needs an IPC handler:

```rust
// Example: VDFS IPC handler
pub async fn handle_vdfs_request(
    method: &str,
    params: JsonValue,
    library: &Library
) -> Result<JsonValue> {
    match method {
        "vdfs.create_entry" => {
            let req: CreateEntryRequest = serde_json::from_value(params)?;
            let entry = library.create_entry(req.into()).await?;
            Ok(json!({ "entry_id": entry.id }))
        }
        "vdfs.write_sidecar" => {
            let req: WriteSidecarRequest = serde_json::from_value(params)?;
            library.write_sidecar(
                &req.entry_id,
                &req.filename,
                &req.data
            ).await?;
            Ok(json!({ "success": true }))
        }
        _ => Err(anyhow::anyhow!("Unknown method: {}", method))
    }
}
```

**Estimated Work:**
- Integration Manager: 1-2 weeks
- IPC protocol + routing: 3-5 days
- Service wrappers: 2-3 days per service (7 services = ~3 weeks)
- **Total: 6-8 weeks for platform foundation**

But we can **parallelize**:
- Team 1: Build integration platform
- Team 2: Build Finance extension (against mocked IPC)
- Week 6: Integration testing

---

## The First Extension: Spacedrive Finance

**Revenue Target:** $10/month, 50K users by 2027 = $500K MRR

**Technical Scope:**

### MVP (3 weeks)
✅ Gmail OAuth
✅ Email scanning (keyword-based)
✅ Entry creation
✅ PDF OCR (Tesseract)
✅ AI classification (local Ollama)
✅ CSV export
✅ Basic UI (receipt list + search)

### V2 (Post-MVP)
❌ Outlook/IMAP support
❌ Multi-currency
❌ QuickBooks API
❌ Mobile scanning
❌ Automatic vendor reconciliation

### Data Flow

```
Gmail → EmailScanJob → Receipt Detection → Entry Creation
                                              ↓
                                         Store email.json
                                              ↓
                                          OcrJob (PDF)
                                              ↓
                                         Store ocr.txt
                                              ↓
                                    AI Classification
                                              ↓
                               Store receipt_analysis.json
                                              ↓
                                  Update Entry Metadata
                                              ↓
                            Auto-index for Search (Event Bus)
```

---

## Success Metrics & Validation

### Phase 1 Success Criteria

**Technical:**
- [ ] Extension runs as separate process
- [ ] Successfully connects to Gmail via OAuth
- [ ] Processes 100 receipts end-to-end
- [ ] <1 second per receipt average
- [ ] <5% OCR/classification errors

**Business:**
- [ ] 1,000 beta signups in Month 1
- [ ] 100 paying users in Month 3 ($1K MRR)
- [ ] <5% monthly churn
- [ ] NPS > 50

**Learning:**
- What's the optimal price point? ($5, $10, $15)
- Which features are must-haves?
- What receipt formats cause problems?
- Do users prefer local AI or cloud API?

### Phase 2 Success Criteria

**Technical:**
- [ ] WASM runtime loads plugins
- [ ] Finance extension migrated to WASM
- [ ] 10+ third-party extensions submitted
- [ ] Hot-reload works during development

**Business:**
- [ ] 10K paying extension users ($120K MRR)
- [ ] 30+ plugins in marketplace
- [ ] $10K+ monthly platform fees (from 3rd party extensions)

---

## Risk Analysis

### Risk 1: Users Won't Pay
**Probability:** Low
**Evidence:** WellyBox has paying customers at similar price
**Mitigation:** Start with high-value, privacy-sensitive category (Finance)

### Risk 2: Integration Platform Takes Too Long
**Probability:** Medium
**Evidence:** 6-8 weeks for robust IPC system
**Mitigation:** Start with minimal viable IPC, iterate based on Finance needs

### Risk 3: WASM Performance Issues
**Probability:** Low-Medium
**Evidence:** WASM overhead is typically <10%
**Mitigation:** Benchmark early, use native modules for heavy computation

### Risk 4: Receipt Detection Accuracy
**Probability:** Medium
**Evidence:** Many receipt formats, OCR can fail
**Mitigation:** Start with major vendors (Starbucks, Amazon), improve incrementally

---

## Next Steps

### Immediate (This Week)

1. **Review with Team**
   - Technical feasibility of IPC approach
   - Resource allocation (who works on what)
   - Timeline validation

2. **Prototype IPC Protocol**
   - Define message format
   - Implement basic client/server
   - Test with dummy extension

3. **Design Integration Manager**
   - Process lifecycle
   - IPC routing
   - Error handling

### Next 2 Weeks

1. **Build Integration Platform**
   - Integration Manager skeleton
   - IPC protocol implementation
   - Basic service wrappers (VDFS, Jobs)

2. **Start Finance Extension**
   - Project structure
   - Gmail OAuth
   - IPC client library

3. **Parallel Development**
   - Platform team: Core IPC services
   - Extension team: Business logic (mock IPC)
   - Week 3: Integration

### Month 2-3

1. **Complete Finance MVP**
   - Full email pipeline
   - OCR + classification
   - UI integration
   - Testing

2. **Beta Launch**
   - 100 hand-picked users
   - Feedback loop
   - Bug fixes

3. **Public Launch**
   - ProductHunt
   - Hacker News
   - Content marketing

---

## Resources

### Documentation
- [Platform Revenue Model](./PLATFORM_REVENUE_MODEL.md) - Full business case
- [Email Extension Technical Design](./core/design/EMAIL_INGESTION_EXTENSION_DESIGN.md) - Implementation details
- [Integration System Design](./core/design/INTEGRATION_SYSTEM_DESIGN.md) - Process-based architecture
- [Whitepaper Section 6.7](../whitepaper/spacedrive.tex#L2590) - WASM plugin architecture

### Tasks
- [PLUG-000: WASM Plugin System Epic](../.tasks/PLUG-000-wasm-plugin-system.md)
- [PLUG-001: Integrate WASM Runtime](../.tasks/PLUG-001-integrate-wasm-runtime.md)
- [PLUG-002: Define VDFS Plugin API](../.tasks/PLUG-002-define-vdfs-plugin-api.md)
- [PLUG-003: Twitter Archive PoC](../.tasks/PLUG-003-develop-twitter-agent-poc.md)

### Reference Implementations
- Obsidian (JavaScript plugins)
- VS Code (Extension API)
- Figma (Plugin system)
- Browser extensions (Chrome/Firefox)

---

## Conclusion

We have:
✅ **Clear business model** (extensions > SaaS marginal costs)
✅ **Technical architecture** (process-based → WASM migration path)
✅ **First extension design** (Finance/receipts with proven market)
✅ **Integration points mapped** (7 core systems, clear APIs)
✅ **Realistic timeline** (3 weeks to MVP, 3 months to revenue)

**The path is clear. Time to build.** 🚀

---

*Last Updated: October 2025*

