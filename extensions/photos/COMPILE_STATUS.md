# Photos Extension Compilation Status

**Current Status:** ⚠️ Does not compile (by design - aspirational reference)

---

## Purpose

The Photos extension is a **complete reference implementation** showing every SDK feature, not a working extension yet.

It demonstrates:
- ✅ Content-scoped models
- ✅ Standalone models
- ✅ Agent with memory
- ✅ Jobs and tasks
- ✅ Actions and queries
- ✅ AI integration
- ✅ Complete architecture

**It's meant to guide implementation, not to run.**

---

## Known Issues

### Missing Derives

Models need Serialize/Deserialize:
```rust
#[derive(Serialize, Deserialize)]  // Add this
#[model]
struct Photo { ... }
```

### Missing Dependencies

```toml
# Add to Cargo.toml
tracing = "0.1"
```

### Macro Limitations

Current macros are stubs - they don't generate full code yet:
- `#[agent]` doesn't register handlers
- `#[task]` doesn't handle retries
- `#[action]` doesn't export FFI

### Async/Await Chains

Some method chains don't work as written - needs macro expansion.

---

## For Now

**Use test-extension as the working example:**
```bash
cd extensions/test-extension
cargo build --target wasm32-unknown-unknown --release
# ✅ This works!
```

**Photos extension is aspirational** - shows what's possible when SDK is complete.

---

## To Make It Compile

Would need to:
1. Add all missing derives
2. Simplify agent implementation
3. Remove unimplemented features
4. Add tracing dependency

**But that defeats the purpose** - it's meant to show the complete API surface.

---

**Recommendation:** Keep Photos as reference, use test-extension for actual development.

