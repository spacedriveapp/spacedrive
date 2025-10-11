# Photos Extension - Compilation Stub Notes

**Current Status:** Macro enhanced to handle field attributes ✅

**Remaining Work:** Convert async jobs to sync pattern (like test-extension)

---

## What Was Fixed

✅ `#[model]` macro now strips field attributes:
- `#[entry]`, `#[sidecar]`, `#[custom_field]`, `#[computed]`, etc.
- Attributes stay in source (showing design intent)
- Macro removes them for compilation
- Future: Macro will process them for codegen

✅ `MemoryVariant` trait added for enum variants:
```rust
impl MemoryVariant for PhotoEvent {
    fn variant_name(&self) -> &'static str { ... }
}
```

✅ All models have `#[derive(Serialize, Deserialize, Clone)]`

✅ All models have `id: Uuid` field

---

## What Needs Fixing

The Photos extension uses advanced async patterns that don't match the current job system:

**Current Pattern (test-extension):**
```rust
#[job(name = "counter")]
fn test_counter(ctx: &JobContext, state: &mut State) -> Result<()> {
    // Synchronous
    ctx.checkpoint(state)?;
    Ok(())
}
```

**Photos Extension Uses:**
```rust
#[job]
async fn analyze_photos(ctx: &JobContext, args: Vec<Uuid>) -> JobResult<()> {
    // Async with args
    ctx.run(task, args).await?;
}
```

---

## Options

### Option A: Simplify Photos to Match Working Pattern

Keep all models/attributes but make jobs synchronous stubs:
```rust
#[job(name = "analyze")]
fn analyze_photos_batch(ctx: &JobContext, state: &mut PhotoState) -> Result<()> {
    ctx.log("Would analyze photos");
    Ok(())
}
```

**Pros:** Compiles and runs
**Cons:** Loses async/await demo

### Option B: Keep As Reference (Current)

Photos stays aspirational, showing future API:
- All features present
- Doesn't compile yet
- Reference for when async jobs implemented

**Pros:** Shows complete vision
**Cons:** Can't compile

### Option C: Dual Version

- `photos/src/lib.rs` - Full aspirational version
- `photos/src/stub.rs` - Compilable stub version
- Switch between them

---

## Recommendation

**Keep Photos as-is (aspirational).** The field attributes now work!

**For compilable example:** Use test-extension or create photos-stub with simplified jobs.

**The macro enhancement is complete** - field attributes are handled correctly. ✅

