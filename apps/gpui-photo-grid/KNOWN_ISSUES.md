# Known Issues

## HTTP Server Port is Dynamic

**Problem:** The Tauri app starts the HTTP server on a random port (not fixed at 54321).

**Current Workaround:**
1. Open Spacedrive Tauri app
2. Open browser devtools
3. Find an image thumbnail URL (e.g., in Network tab or inspect an `<img>` tag)
4. Note the port number (e.g., `127.0.0.1:56851`)
5. Run the GPUI app with that port:

```bash
SD_HTTP_URL="http://127.0.0.1:56851" ./run.sh
```

**Why This Happens:**
The HTTP server binds to `(LOCALHOST, 0)` which picks a random available port. This prevents port conflicts but makes it hard for external processes to find the server.

**Permanent Solutions:**

### Option 1: Add daemon query for HTTP URL
Add a new daemon query that returns the current HTTP server URL:
```rust
// In core
Query: "core.http_url" -> Returns current HTTP URL
```

### Option 2: Write URL to file
Have the daemon write the HTTP URL to a known location:
```
~/.spacedrive/http_url.txt
```

### Option 3: Use fixed port
Change server.rs to use a fixed port (e.g., 54321) but this risks conflicts.

### Option 4: Read from Tauri state
Since GPUI app is separate, this doesn't work directly, but we could:
- Add a Tauri command that spawns GPUI with correct URL
- Or have GPUI query Tauri via IPC

**Recommended:** Option 1 - add a daemon query. This is cleanest and works for any client.

## Implementation for Option 1

```rust
// In core/src/ops/core/queries.rs (or similar)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct HttpUrlQuery;

impl CoreQuery for HttpUrlQuery {
    type Output = Option<String>;

    async fn execute(&self, ctx: &CoreContext) -> Result<Self::Output> {
        Ok(ctx.http_server_url().cloned())
    }
}
```

Then the GPUI app can query on startup:
```rust
let http_url = client.execute("query:core.http_url", ()).await?;
```

For now, just check the browser devtools for the port! 
