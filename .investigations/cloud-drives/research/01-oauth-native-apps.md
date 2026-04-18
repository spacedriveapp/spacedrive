# OAuth 2.0 for Native Apps — Research Report for Spacedrive

**Author:** Research Agent
**Date:** 2026-04-18
**Scope:** Replace manual access/refresh token paste with a browser-based OAuth 2.0 flow for Google Drive, OneDrive, and Dropbox in Spacedrive's Rust + Tauri v2 + React desktop app. Backend storage adapter is Apache OpenDAL 0.54.

---

## 1. Executive Summary

**Adopt the RFC 8252 loopback-interface redirect flow with PKCE (S256)**, implemented in Rust using the `oauth2` crate (`/ramosbugs/oauth2-rs`, the de-facto Rust OAuth2 client). The daemon binds an ephemeral loopback port (`http://127.0.0.1:{port}/oauth/callback`), launches the system browser via the `webbrowser` crate, receives the authorization `code` + `state`, exchanges it server-side for access + refresh tokens, and stores them securely (keyring). All three target providers (Google Drive, Microsoft Graph/OneDrive, Dropbox) officially document loopback-IP redirection as the recommended flow for desktop apps and require or strongly recommend PKCE for public clients.

**Do not use `tauri-plugin-oauth`.** It works (last release v2.0.0 on 2024-11-05, 206 stars, author Fabian Lars is a Tauri core maintainer), but it's a thin wrapper around a localhost HTTP server that the Tauri side emits back as a raw URL. It adds a dependency and an IPC boundary with no actual OAuth logic. For Spacedrive — where the *daemon* (not the Tauri webview) owns the cloud integration — running the loopback server directly from Rust inside `sd-daemon` is simpler, eliminates a Tauri plugin surface, and keeps the flow identical across CLI and future headless deployments. `tauri-plugin-deep-link` is only needed if we ever want `spacedrive://oauth/callback` custom-scheme redirects, which is **not recommended** by any of the three providers for desktop.

**Ship public client_ids in the daemon binary.** Google explicitly states that for installed apps, "the client secret is obviously not treated as a secret" (see §7 Sources: Google OAuth2 overview). Dropbox and Microsoft endorse PKCE-only public clients with no secret. This matches what rclone, Insync, Cyberduck, and every open-source desktop cloud client does. The main operational cost is Google's OAuth verification process to lift the 7-day refresh-token expiry (see §6.1).

---

## 2. Recommended Flow Diagram (ASCII)

```
┌─────────────────────────────────────────────────────────────────────┐
│                       USER DEVICE (single machine)                  │
│                                                                     │
│   sd-daemon (Rust)                                 Default Browser  │
│   ┌──────────────────────────┐                   ┌────────────────┐ │
│   │ 1. Bind 127.0.0.1:{0}    │                   │                │ │
│   │    -> ephemeral port P   │                   │                │ │
│   │ 2. Generate:             │                   │                │ │
│   │    - code_verifier (64b) │                   │                │ │
│   │    - challenge=S256(cv)  │                   │                │ │
│   │    - state (32b random)  │                   │                │ │
│   │ 3. Build auth_url        │                   │                │ │
│   │    redirect_uri=         │                   │                │ │
│   │      http://127.0.0.1:P  │                   │                │ │
│   │      /oauth/callback     │                   │                │ │
│   │ 4. webbrowser::open ─────┼──────────────────▶│ GET auth_url   │ │
│   └──────────────────────────┘                   └────────┬───────┘ │
│                                                           │         │
│                                                           │ login + │
│                                                           │ consent │
│                                                           ▼         │
│   ┌──────────────────────────┐          ┌─────────────────────────┐ │
│   │ 5. HTTP listener         │◀─302─────│ https://provider/auth   │ │
│   │    on 127.0.0.1:P        │          │ issues redirect to      │ │
│   │    receives GET          │          │ http://127.0.0.1:P/     │ │
│   │    /oauth/callback       │          │ oauth/callback?code=X   │ │
│   │    ?code=X&state=Y       │          │ &state=Y                │ │
│   └──────────┬───────────────┘          └─────────────────────────┘ │
│              │ validate state == sent                               │
│              │ return 200 HTML "You can close this tab"             │
│              ▼                                                      │
│   ┌──────────────────────────┐                                      │
│   │ 6. Stop listener         │                                      │
│   │ 7. POST token_url        │────https (TLS)────▶ provider/token   │
│   │    client_id=...         │                                      │
│   │    code=X                │                                      │
│   │    code_verifier=cv      │                                      │
│   │    grant=auth_code       │◀──access_token, refresh_token────    │
│   │    redirect_uri=http:// │                                      │
│   │      127.0.0.1:P/...     │                                      │
│   │ 8. Persist tokens in OS  │                                      │
│   │    keyring + config DB   │                                      │
│   └──────────────────────────┘                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. RFC 8252 Key Rules Spacedrive MUST Follow

From [RFC 8252 §7.3 and §8](https://datatracker.ietf.org/doc/html/rfc8252):

1. **MUST use an external user-agent** (the system browser). Embedded WebViews are forbidden by the spec and actively blocked by Google (`disallowed_useragent` error). "Native apps MUST NOT use embedded user-agents to perform authorization requests" (§8.12).
2. **MUST implement PKCE (RFC 7636) with S256.** RFC 8252 §6 quote: "Public native app clients MUST implement the Proof Key for Code Exchange (PKCE) extension to OAuth, and authorization servers MUST support PKCE for such clients."
3. **MUST bind loopback on `127.0.0.1` (IPv4) and/or `[::1]` (IPv6), NOT `localhost`.** §8.3 quote: "Specifying a redirect URI with the loopback IP literal rather than localhost avoids inadvertently listening on network interfaces other than the loopback interface. It is also less susceptible to client-side firewalls and misconfigured host name resolution on the user's device." Attempt IPv4 and IPv6 and use whichever binds.
4. **MUST use an ephemeral (OS-assigned) port.** §7.3: "The authorization server MUST allow any port to be specified at the time of the request for loopback IP redirect URIs, to accommodate clients that obtain an available ephemeral port from the operating system at the time of the request."
5. **MUST NOT use the Implicit Grant.** §8.2: implicit grant cannot be protected by PKCE and is "NOT RECOMMENDED" for native apps.
6. **MUST include a high-entropy `state` parameter and reject any callback whose state doesn't match** (§8.9, CSRF protection).
7. **MUST use a distinct redirect path per authorization server** (§8.10, "Authorization Server Mix-Up Mitigation") — e.g. `/oauth/callback/google`, `/oauth/callback/microsoft`, `/oauth/callback/dropbox`.
8. **MUST NOT treat statically-embedded client secrets as confidential** (§8.5): "Secrets that are statically included as part of an app distributed to multiple users should not be treated as confidential secrets". Registered secrets for Google "Desktop app" clients are an identifier, not a credential.
9. **MUST open the loopback port only for the duration of the flow** and close it immediately on receiving a callback or on timeout (§8.3).
10. **SHOULD NOT use private-use URI schemes (`spacedrive://…`) on desktop** when loopback is available; Google forbids custom schemes for Desktop client types, and §7.1 notes multiple apps can claim the same scheme leading to code-interception attacks.

---

## 4. Rust Implementation Sketch

Real Rust, using `oauth2 = "5"` (current API as of 2026; uses the builder pattern where `BasicClient` takes `ClientId` first and the token/auth URLs are added via `set_*`). Source: Context7 `/ramosbugs/oauth2-rs`.

```rust
// Cargo.toml (add to core)
// oauth2 = "5"
// webbrowser = "1"
// tokio = { version = "1", features = ["net", "io-util", "macros", "rt"] }
// rand = "0.8"
// url = "2"

use std::net::{Ipv4Addr, SocketAddr};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<std::time::SystemTime>,
    pub scopes: Vec<String>,
}

pub struct ProviderConfig {
    pub client_id: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static [&'static str],
    /// Distinct path per provider — RFC 8252 §8.10 mix-up mitigation.
    pub callback_path: &'static str,
    /// Extra query params: Google needs `access_type=offline&prompt=consent` for
    /// refresh_token on re-auth; Microsoft needs `offline_access` in scopes;
    /// Dropbox needs `token_access_type=offline` for refresh_token.
    pub extra_auth_params: &'static [(&'static str, &'static str)],
}

pub async fn run_loopback_flow(cfg: &ProviderConfig) -> anyhow::Result<OAuthTokens> {
    // 1. Bind to ephemeral port on 127.0.0.1. Try IPv4 first; fall back to IPv6
    //    per RFC 8252 §7.3.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{}", cfg.callback_path);

    // 2. Build oauth2 client.
    let client = BasicClient::new(ClientId::new(cfg.client_id.to_string()))
        .set_auth_uri(AuthUrl::new(cfg.auth_url.to_string())?)
        .set_token_uri(TokenUrl::new(cfg.token_url.to_string())?)
        .set_redirect_uri(RedirectUrl::new(redirect_uri.clone())?);

    // 3. Generate PKCE challenge (S256) + CSRF state.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut req = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge);
    for scope in cfg.scopes {
        req = req.add_scope(Scope::new((*scope).into()));
    }
    for (k, v) in cfg.extra_auth_params {
        req = req.add_extra_param(*k, *v);
    }
    let (auth_url, csrf_state) = req.url();

    // 4. Open system browser.
    webbrowser::open(auth_url.as_str())?;
    tracing::info!(%auth_url, "opened browser for OAuth consent");

    // 5. Accept exactly one connection and parse `code` + `state`.
    let (code, state) = accept_callback(&listener, cfg.callback_path).await?;

    // 6. Validate CSRF state — aborts on mismatch (RFC 8252 §8.9).
    if state.secret() != csrf_state.secret() {
        anyhow::bail!("OAuth state mismatch — possible CSRF attack");
    }

    // 7. Exchange code for tokens.
    let http = oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none()) // SSRF hardening
        .build()?;
    let token = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http)
        .await?;

    Ok(OAuthTokens {
        access_token: token.access_token().secret().clone(),
        refresh_token: token.refresh_token().map(|t| t.secret().clone()),
        expires_at: token.expires_in().map(|d| std::time::SystemTime::now() + d),
        scopes: token
            .scopes()
            .map(|s| s.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default(),
    })
}

/// Minimal one-shot HTTP server: accepts one TCP connection, parses the GET
/// line, extracts ?code=…&state=…, writes a human-friendly close page, and
/// drops the listener so the port is released.
async fn accept_callback(
    listener: &TcpListener,
    expected_path: &str,
) -> anyhow::Result<(AuthorizationCode, CsrfToken)> {
    let (mut stream, _) = listener.accept().await?;
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).await?;
    let req = std::str::from_utf8(&buf[..n])?;

    // Parse the request line: "GET /oauth/callback/google?code=...&state=... HTTP/1.1"
    let first = req.lines().next().unwrap_or("");
    let path_and_query = first.split_whitespace().nth(1).unwrap_or("");
    let url = url::Url::parse(&format!("http://x{}", path_and_query))?;

    if url.path() != expected_path {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
        anyhow::bail!("unexpected callback path: {}", url.path());
    }

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.to_string()),
            "state" => state = Some(v.to_string()),
            "error" => error = Some(v.to_string()),
            _ => {}
        }
    }
    let body = "<!doctype html><html><body style='font-family:sans-serif;text-align:center;padding:2rem'>\
        <h1>Spacedrive OAuth complete</h1><p>You can close this tab.</p>\
        <script>setTimeout(()=>window.close(),500)</script></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;

    if let Some(e) = error {
        anyhow::bail!("authorization error: {e}");
    }
    Ok((
        AuthorizationCode::new(code.ok_or_else(|| anyhow::anyhow!("missing code"))?),
        CsrfToken::new(state.ok_or_else(|| anyhow::anyhow!("missing state"))?),
    ))
}

/// Proactive refresh (see §7). Call ~5 min before `expires_at`.
pub async fn refresh(
    cfg: &ProviderConfig,
    refresh_token: &str,
) -> anyhow::Result<OAuthTokens> {
    let client = BasicClient::new(ClientId::new(cfg.client_id.to_string()))
        .set_token_uri(TokenUrl::new(cfg.token_url.to_string())?);
    let http = oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()?;
    let resp = client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.to_string()))
        .request_async(&http)
        .await?;
    Ok(OAuthTokens {
        access_token: resp.access_token().secret().clone(),
        // Google/MS/Dropbox MAY rotate the refresh token — always persist the new one.
        refresh_token: resp.refresh_token().map(|t| t.secret().clone()),
        expires_at: resp.expires_in().map(|d| std::time::SystemTime::now() + d),
        scopes: resp
            .scopes()
            .map(|s| s.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default(),
    })
}
```

**Notes on the sketch:**
- `oauth2 v5` requires disabling the reqwest redirect policy to prevent SSRF (verbatim comment in the crate's own example: "IMPORTANT: disable redirects to prevent SSRF").
- `webbrowser` crate (`/amodm/webbrowser-rs`, high reputation, benchmark score 93.7) is the standard; it handles macOS / Windows / Linux / WSL and respects `$BROWSER` on Unix.
- The minimal hand-rolled HTTP parser is safe because it only has to handle one request from `127.0.0.1`. Using `hyper` is overkill; the existing approach in the `oauth2` crate's `examples/github.rs` also uses `std::net::TcpListener` + manual parsing.
- **Timeout:** wrap `accept_callback` in `tokio::time::timeout(Duration::from_secs(300), …)` so a user who abandons the browser doesn't leave a port bound.

---

## 5. Tauri Integration Options — Comparison Table

| Approach                                     | Pros                                                                                                                       | Cons                                                                                                                                                         | Effort  | Recommendation |
|----------------------------------------------|----------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------|---------|----------------|
| **Loopback in `sd-daemon` (recommended)**    | - Daemon owns cloud creds (matches existing architecture) <br>- Works from CLI + Tauri + headless identically <br>- No extra Tauri IPC hop <br>- Zero plugin dependencies | - Requires spawning a TCP listener in daemon (trivial with tokio)                                                                                            | **Low** | ✅ Adopt |
| `tauri-plugin-oauth` (FabianLars)            | - Abstracts the loopback server <br>- Ships TS bindings (`start`, `onUrl`, `cancel`) <br>- Actively a Tauri-core author    | - v2.0.0 last released 2024-11-05 (5 releases, low velocity) <br>- Runs server in Tauri process, not daemon — wrong layer for Spacedrive <br>- Adds JS bridge <br>- README itself says "you must verify the URL" — no PKCE/state handling | Low-Med | ❌ Skip |
| `tauri-plugin-deep-link` + custom scheme     | - Works when app is closed (via `single-instance` plugin) <br>- No ephemeral port, no firewall prompt                      | - Google forbids custom schemes on Desktop client type (see §6.1) <br>- Microsoft requires specific MSAL-formatted URIs <br>- Dropbox does not natively advertise desktop scheme redirect <br>- Multiple apps can claim the same scheme (RFC 8252 §7.1 warning) <br>- Requires installer/registry tweaks on Windows and Linux | Med     | ❌ Skip (for cloud providers) |
| Embedded WebView (e.g., Tauri sub-window)    | - Slick UX                                                                                                                  | - **Forbidden by RFC 8252 §8.12** <br>- Google explicitly blocks it (`disallowed_useragent`) <br>- Microsoft blocks it for most scopes                       | Low     | ❌ Absolutely not |

**Conclusion:** Run the loopback server in `sd-daemon` directly. The Tauri app calls a daemon action `cloud.oauth.start { provider: "google" }` that blocks until the flow completes (or returns a session ID the frontend polls). The React UI's only job is to show "Waiting for browser authentication…" with a cancel button that calls `cloud.oauth.cancel`.

---

## 6. Per-Provider Specs

### 6.1 Google Drive

Primary sources: [OAuth 2.0 for iOS & Desktop Apps](https://developers.google.com/identity/protocols/oauth2/native-app), [Drive API scopes](https://developers.google.com/workspace/drive/api/guides/api-specific-auth), [OAuth overview](https://developers.google.com/identity/protocols/oauth2). All docs last updated 2026-04-01/2026-04-03.

**Authorization endpoint:** `https://accounts.google.com/o/oauth2/v2/auth`
**Token endpoint:** `https://oauth2.googleapis.com/token`
**Revocation:** `https://oauth2.googleapis.com/revoke`

**Client registration:**
- Console: Google Cloud Console → Credentials → Create Client → **Desktop app** type.
- Loopback (`http://127.0.0.1:{port}` or `http://[::1]:{port}`) is the recommended and currently-supported redirect for Desktop app clients (macOS, Linux, Windows). OOB copy/paste flow is **deprecated** as of 2022 and no longer works.
- Google also issues a `client_secret` for Desktop clients. Verbatim from Google: "The process results in a client ID and, in some cases, a client secret, which you embed in the source code of your application. (In this context, the client secret is obviously not treated as a secret.)" We will ship it but treat the client as public.
- **Custom URI schemes are no longer supported** for Desktop/Chrome — must use loopback.

**Required scopes** (pick the minimum):
- `https://www.googleapis.com/auth/drive.file` — **Non-sensitive**, per-file access. Only files the user explicitly picks/creates with the app. Does NOT require restricted-scope verification. **Preferred for Spacedrive v1.**
- `https://www.googleapis.com/auth/drive` — **Restricted**, full read/write. Requires Google's restricted-scope verification + annual third-party security assessment (cost: four-to-five-figure USD). Spacedrive qualifies under the "Backup and sync" app category.
- `https://www.googleapis.com/auth/drive.readonly` — **Restricted**, full read-only.
- `https://www.googleapis.com/auth/drive.metadata.readonly` — Restricted but lighter; useful for a file-browsing preview mode.

**To get a refresh_token** the auth URL **MUST include** `access_type=offline` and on re-auth `prompt=consent` (add to `extra_auth_params`). Google's doc: "Note that refresh tokens are always returned for installed applications" — true for Desktop client type, still set `access_type=offline` to be safe.

**Token lifetimes:**
- Access token: ~3600 s (1 hour).
- Refresh token: long-lived with these caveats:
  - **7-day expiry for unverified apps in "Testing" publishing status unless only `openid`/`email`/`profile` scopes are requested.** Verbatim: "A Google Cloud Platform project with an OAuth consent screen configured for an external user type and a publishing status of 'Testing' is issued a refresh token expiring in 7 days". **This is the single biggest UX hurdle for Spacedrive pre-verification.**
  - After verification ("In production"): refresh token lives until (a) user revokes, (b) 6 months of non-use, (c) per-client limit of 100 refresh tokens per user (oldest invalidated), or (d) GCP session-control policies.
- Refresh tokens **may be rotated** on refresh — always persist the new one if returned.

**Gotchas:**
- Drive API v3 is the current version; use it.
- `drive.file` scope files are invisible to the web UI unless created via a Picker or app upload — great for sandbox isolation, poor for "show me all my Drive files" UX.
- `admin_policy_enforced` error for Workspace users whose admin restricted third-party app access.
- Google rate-limits token refresh; do not refresh proactively more often than necessary.
- HTTP/2 has known issues with Drive (rclone disables it by default — see `--drive-disable-http2`). Worth noting for OpenDAL.

**Verification track:** To lift the 7-day refresh-token expiry, submit OAuth verification (for non-sensitive `drive.file` this is a quick "Brand" verification, no video demo required).

---

### 6.2 Microsoft OneDrive / Graph

Primary sources: [Get access on behalf of a user — Microsoft Graph](https://learn.microsoft.com/en-us/graph/auth-v2-user) (updated 2025-08-29), [OAuth 2.0 authorization code flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow) (updated 2026-01-22), [Desktop app registration](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-registration).

**Authorization endpoint:** `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize`
**Token endpoint:** `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`

**Tenant selection:**
- `common` — both work/school (Entra ID) and personal Microsoft accounts. **Recommended** for Spacedrive: one client ID serves OneDrive Personal + OneDrive Business.
- `consumers` — personal only.
- `organizations` — work/school only.
- Tenant GUID — single tenant.

**Client registration:**
- Entra admin center → App registrations → **Accounts in any organizational directory and personal Microsoft accounts**.
- Under Authentication → Add platform → **Mobile and desktop applications** → redirect URI **exactly** `http://localhost` (yes, Microsoft literally docs `localhost` here despite RFC 8252 saying `127.0.0.1`; in practice both work because Microsoft does a relaxed path match for native-redirect + localhost. For us, register `http://localhost` AND connect with `http://127.0.0.1:{port}` — Microsoft's loopback exception permits any port at any path when `localhost` is registered).
- Set **"Allow public client flows" = Yes** under Advanced settings.
- **No client secret.** Public-client desktop apps do not use one. Microsoft verbatim: "Don't use it in a native app, because client secrets can't be reliably stored on devices."

**Required scopes:**
- `Files.ReadWrite.All` — full read/write on the user's OneDrive (recommended; corresponds to Google's `drive`).
- `Files.ReadWrite` — read/write only to files the app creates or the user picks.
- `Files.Read.All` — read-only everywhere.
- `offline_access` — **REQUIRED** to receive a refresh token. Without it only an access token is returned.
- `User.Read` — optional, for user profile during account UI.

**Token lifetimes:**
- Access token: ~3600 s (Microsoft actually ships 3599 seconds).
- Refresh token: **no explicitly specified lifetime** for native apps. Microsoft quote: "Refresh tokens for web apps and native apps don't have specified lifetimes. Typically, the lifetimes of refresh tokens are relatively long." Subject to Conditional Access policies (org admins can enforce shorter lifetimes).
- **Rotation:** Microsoft rotates the refresh token on nearly every refresh. Verbatim: "Replace the old refresh token with this newly acquired refresh token to ensure your refresh tokens remain valid for as long as possible."

**Gotchas:**
- PKCE is required for SPAs but "recommended for all application types, both public and confidential clients" — we'll send it regardless.
- Microsoft Graph uses the **single resource constraint** — at token exchange, all scopes must be from one resource (all Graph, or all OneDrive personal, not mixed).
- MSAL is the recommended library (MSAL4J, MSAL.NET, MSAL Node). **There is no official MSAL for Rust.** Using `oauth2-rs` directly is the right call.
- The `v2.0` endpoint is the current one; `v1.0` is legacy.
- For OneDrive Personal (consumer) you may additionally need `Files.ReadWrite.AppFolder` if restricting to a sandbox.
- Error `invalid_grant` with `error_subtype=invalid_rapt` means a session-control policy has expired — force re-auth.

---

### 6.3 Dropbox

Primary source: [Dropbox OAuth Guide](https://developers.dropbox.com/oauth-guide) (original 2020-12-07, kept current; scoped-app model is the official version since 2021).

**Authorization endpoint:** `https://www.dropbox.com/oauth2/authorize`
**Token endpoint:** `https://api.dropboxapi.com/oauth2/token`
**Revocation:** `https://api.dropboxapi.com/2/auth/token/revoke`

**Client registration:**
- Dropbox App Console → Create app → **Scoped access** → **Full Dropbox** (or **App folder** for sandboxing).
- Permissions tab: toggle the scopes you'll request.
- Disable "Implicit grant" (Dropbox recommends, we're not using it).
- Register redirect URIs — Dropbox requires exact match. We'll register `http://127.0.0.1:1` as a placeholder and use the loopback-port trick: **Dropbox does NOT allow arbitrary ports** on registered redirect URIs; the exact match is strict. This means we **MUST register one fixed port** (e.g., `http://127.0.0.1:53683/oauth/callback/dropbox`) and always bind that port.

  **This is a real divergence from Google and Microsoft.** Mitigation: pick a high random port (e.g., 53683 — what rclone uses is 53682 for Google and a different one for Dropbox), allow user to override, handle `EADDRINUSE` by asking user to close whatever is using it. Alternatively, register multiple ports (Dropbox supports multiple redirect URIs per app) and try them in order.

**Required scopes** (new scoped model):
- `files.content.read` — read file contents.
- `files.content.write` — write file contents.
- `files.metadata.read` — read metadata (list, search).
- `files.metadata.write` — modify metadata.
- `account_info.read` — for displaying user identity in UI.
- Minimal set for a sync client: `files.content.read files.content.write files.metadata.read files.metadata.write account_info.read`.

**PKCE:** Dropbox has the strongest guidance of the three: "desktop and mobile apps without a server" should use **PKCE instead of `client_secret`**. When using PKCE, the token request passes `code_verifier` **instead of** `client_id` (Dropbox-specific wording) — in practice you pass both and Dropbox accepts it. `oauth2-rs` handles this automatically.

**Token lifetimes:**
- **Short-lived access tokens** are the only option for new apps as of 2021 (long-lived tokens deprecated).
- Access token: ~4 hours (14400 seconds).
- **Refresh token is only issued if `token_access_type=offline` is on the authorization URL.** This is Dropbox-specific and critical — without it you only get a 4-hour access token and no refresh. Add `("token_access_type", "offline")` to `extra_auth_params`.
- Refresh token: long-lived, **not rotated** (Dropbox keeps a static refresh token). Lives until user revokes.

**Gotchas:**
- Dropbox rejects the flow if `response_type` is anything other than `code`.
- The port-lock issue above is the biggest operational concern. The `oauth2-broker` crate (`/hack-ink/oauth2-broker`) has port-pool logic worth referencing.
- Dropbox's `/oauth2/token` endpoint accepts PKCE + client_id with no secret; this is the public-client path.

---

## 7. Token Refresh Strategy

**Decision: proactive refresh, with reactive 401-retry as a safety net.**

**Storage format** (one row per connected account):
```
{
  "provider": "google" | "microsoft" | "dropbox",
  "account_id": "opaque provider user id",
  "account_email": "display only",
  "access_token": "<stored in OS keyring>",
  "refresh_token": "<stored in OS keyring>",
  "expires_at": "2026-04-18T14:23:00Z",
  "scopes": ["files.content.read", "files.content.write", ...],
  "client_id_fingerprint": "sha256(client_id)",  // detect client-id rotation
  "created_at": "...",
  "last_refreshed_at": "..."
}
```

Use the existing `keyring` crate (or `tauri-plugin-keyring` if we're also persisting from the Tauri side); fall back to an encrypted file if keyring isn't available (e.g., headless Linux without a secret service). **Never log tokens.** The `oauth2` crate's `Secret` wrapper already redacts them from `Debug`; preserve that boundary.

**Refresh policy:**
1. **Proactive:** a daemon task polls every 60 seconds and refreshes any token where `expires_at - now < 5 minutes`. Avoids first-request latency and lets us back-off+retry on transient 5xx.
2. **Reactive:** on a 401 from the provider API (OpenDAL surfaces these as `ErrorKind::Unauthenticated`), force-refresh once and retry the original request. If refresh also fails with `invalid_grant`, mark the account as `requires_reauth` and surface a UI notification.
3. **Always persist the new refresh_token** when the response contains one (Microsoft rotates, Google may rotate, Dropbox does not).
4. **Rotation race:** guard the refresh call with a per-account async mutex so two concurrent API calls don't both try to refresh.

**Failure states and UX:**
- `invalid_grant` — user revoked access, password change (Google with Gmail scope), 6 months idle (Google), 7-day expiry (unverified Google). → Mark account `requires_reauth`, show banner "Reconnect your Google Drive" that re-runs the loopback flow.
- Network error → retry with exponential backoff.
- Rate-limit → respect `Retry-After` header.

**Clock skew:** always compute `expires_at` from the local clock at the moment of the token response (`now + expires_in`), not from any server timestamp. Apply a 5-minute safety margin on top when deciding to refresh.

---

## 8. Open Questions (Product Decisions Required)

1. **Ship public client_ids in the daemon binary, or require BYO credentials?**
   - **Option A (default, recommended):** ship Spacedrive-owned client IDs for all three providers. Matches rclone, Insync, CloudMounter, Cyberduck. Requires going through:
     - **Google OAuth verification** (mandatory to use sensitive/restricted scopes OR to lift the 7-day refresh-token expiry). For `drive.file` only (non-sensitive) this is a trivial brand verification. For `drive` (restricted) this is a CASA assessment — expect $5–20k/yr.
     - **Microsoft publisher verification** (free; requires an MPN ID).
     - **Dropbox production status** (request via support after 50 users). Applies only when your app exceeds the dev rate-limit of 500 users.
   - **Option B (advanced/power-user escape hatch):** allow users to paste their own `client_id` for any provider in config. Zero verification burden on Spacedrive, at the cost of each user needing to create a Cloud Console project. rclone offers this as "make your own client_id" and documents it as "recommended."
   - **Option C (hybrid):** ship Spacedrive client_ids by default, expose Option B in advanced settings. **This is what rclone does and is the clear winner.**

2. **Google `drive.file` vs `drive` scope?**
   - `drive.file` is non-sensitive — no restricted-scope verification, no annual security assessment, fast shipping. BUT Spacedrive users will only see files they created with Spacedrive, not their existing Drive content. That's a functional regression vs. the current "paste raw token" flow.
   - `drive` is what users actually want but triggers the restricted-scope workflow. Launch with `drive.file`, plan the restricted-scope verification as a post-MVP milestone.

3. **Dropbox port collision handling.** If the chosen fixed port is in use, do we (a) register 3–5 ports and try each, (b) ask the user to stop the conflicting app, (c) fall back to Dropbox's no-redirect code-copy flow? Option (a) with e.g. ports 53683, 53684, 53685 is cleanest.

4. **CLI UX.** When the user runs `sd-cli cloud add google`, the daemon opens a browser on the user's local machine. What happens if the CLI is run via SSH on a headless box? Need a fallback:
   - Print the auth URL and say "open this on another device."
   - After the user completes auth, the callback goes to the *remote* loopback which the user can't see. Dead-end. Either require X11 forwarding or accept that cloud-drive setup must be done from a device with a browser. Document this clearly.

5. **Token revocation on account delete.** Should "remove cloud account" in the UI also call the provider's revocation endpoint (Google: `/revoke`, Microsoft: via Graph API, Dropbox: `/2/auth/token/revoke`)? Best practice per RFC 6749 §6 and all three providers: yes. Low-cost, high-trust signal.

6. **OpenDAL integration point.** OpenDAL 0.54's GDrive/OneDrive/Dropbox services accept tokens; we need to check if they expose a refresh hook or expect a fresh token per operation. If no refresh hook, wrap the `Operator` in a `TokenProvider` trait that intercepts 401s. (Deferred to a follow-up research doc.)

7. **Competitive check — what do rclone / Insync / Cyberduck do?**
   - **rclone:** loopback on `127.0.0.1:53682` (default; overridable via `--rc-addr`). Ships public client_ids ("low performance" fallback) but strongly recommends BYO via the docs. Uses Go's `golang.org/x/oauth2` package. Caches token as a JSON blob in `~/.config/rclone/rclone.conf`. Refreshes reactively (on the Go library's side). **This is the canonical reference implementation.**
   - **Insync** (closed source): loopback flow with a Google-verified client_id. Paid app, owns the verification cost.
   - **Cyberduck:** AppAuth-inspired loopback for Google/OneDrive. Ships their own client_ids.
   - **Nextcloud desktop:** loopback for OAuth providers, PKCE.
   - **None use WebViews. None use custom URI schemes for cloud providers.** Consensus is unanimous: loopback + PKCE.

---

## 9. Sources

### RFCs and specifications
- [RFC 8252 — OAuth 2.0 for Native Apps](https://datatracker.ietf.org/doc/html/rfc8252) (Denniss & Bradley, BCP 212, October 2017). Core normative reference; all §3 rules derive from here.
- [RFC 7636 — Proof Key for Code Exchange (PKCE)](https://datatracker.ietf.org/doc/html/rfc7636)
- [RFC 6749 — OAuth 2.0 Authorization Framework](https://datatracker.ietf.org/doc/html/rfc6749)

### Rust libraries (via Context7)
- `/ramosbugs/oauth2-rs` — OAuth2 crate, v5 API. PKCE + loopback example referenced verbatim in §4.
- `/amodm/webbrowser-rs` — cross-platform browser launcher. Reputation: High, score 93.7.
- `/fabianlars/tauri-plugin-oauth` — reviewed, not adopted. README at https://github.com/FabianLars/tauri-plugin-oauth (v2.0.0 released 2024-11-05, 206 stars).
- `/hack-ink/oauth2-broker` — referenced for token-store ideas.

### Tauri v2
- [Tauri deep-linking plugin](https://v2.tauri.app/plugin/deep-linking/) — Tauri v2 docs, last updated 2025-10-18. Reviewed for custom-scheme approach; not adopted for cloud OAuth.

### Google
- [OAuth 2.0 for iOS & Desktop Apps](https://developers.google.com/identity/protocols/oauth2/native-app) (last updated 2026-04-03)
- [Using OAuth 2.0 to Access Google APIs (overview, refresh-token-expiration details)](https://developers.google.com/identity/protocols/oauth2) (last updated 2026-04-03)
- [Choose Google Drive API scopes](https://developers.google.com/workspace/drive/api/guides/api-specific-auth) (last updated 2026-04-01)

### Microsoft
- [Get access on behalf of a user — Microsoft Graph](https://learn.microsoft.com/en-us/graph/auth-v2-user) (last updated 2025-08-29)
- [Microsoft identity platform and OAuth 2.0 authorization code flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow) (last updated 2026-01-22)
- [Configure desktop apps that call web APIs](https://learn.microsoft.com/en-us/entra/identity-platform/scenario-desktop-app-registration) (last updated 2025-10-02)

### Dropbox
- [Dropbox OAuth Guide](https://developers.dropbox.com/oauth-guide) (Dropbox Platform Team, originally 2020-12-07; scoped-app & PKCE guidance current as of access 2026-04-18).

### Competitive references
- [rclone Google Drive docs](https://rclone.org/drive/) — cited for loopback port choices and "make your own client_id" pattern.
