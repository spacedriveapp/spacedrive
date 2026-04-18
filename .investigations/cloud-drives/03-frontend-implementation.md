# Cloud Drives — Frontend / TypeScript / Tauri Investigation

Scope: `apps/tauri/`, `apps/web/`, `apps/mobile/`, `packages/ts-client/`, `packages/interface/`, `packages/ui/`, `packages/assets/`.
No code was modified.

---

## 1. Executive Summary

**Verdict: PARTIAL — plumbing is in place, UX is not.**

The cloud-drive frontend is a single dialog component (`AddStorageModal.tsx`) that lets a user paste raw S3/GCS/Azure credentials or pre-obtained Google/Dropbox/OneDrive OAuth tokens and creates a `Cloud` volume + location. The full Rust type surface (`CloudServiceType`, `CloudStorageConfig`, `VolumeAddCloudInput`, `SdPath::Cloud`) is mirrored into TypeScript and consumed end-to-end — the breadcrumb, PathBar, FileInspector, and FileOperationModal all handle `'Cloud' in sdPath`. However there is **no interactive OAuth flow, no dedicated settings page, no "connected accounts" inventory, no cloud-specific sidebar group, no onboarding step, and no dedicated remove/reconnect UI**. A user currently has to know their OAuth `client_id`, `client_secret`, `access_token`, and `refresh_token` out-of-band, which makes Google Drive / Dropbox / OneDrive effectively developer-only.

---

## 2. Generated Type Surface

Source: `packages/ts-client/src/generated/types.ts` (generated from Rust).

| Name | File:Line | Fields / Shape | Notes |
| --- | --- | --- | --- |
| `CloudServiceType` | generated/types.ts:184 | `"s3" \| "gdrive" \| "dropbox" \| "onedrive" \| "gcs" \| "azblob" \| "b2" \| "wasabi" \| "spaces" \| "cloud"` | `"cloud"` variant is pCloud. No WebDAV/Mega/Box despite icon assets being present. |
| `CloudStorageConfig` | generated/types.ts:186-202 | Tagged union: `S3 \| GoogleDrive \| OneDrive \| Dropbox \| AzureBlob \| GoogleCloudStorage` | S3 takes bucket/region/keys/endpoint. GoogleDrive and OneDrive require `access_token + refresh_token + client_id + client_secret`. Dropbox requires only `refresh_token + client_id + client_secret` (OpenDAL obtains access tokens). GCS takes a service-account JSON blob in `credential`. |
| `SdPath::Cloud` | generated/types.ts:3505-3519 | `{ service: CloudServiceType; identifier: string; path: string }` | `identifier` is the bucket/drive/container name. This is the canonical way to address any file living in cloud storage. |
| `Volume.cloud_identifier` | generated/types.ts:4627-4631 | `string \| null` | Separate from `mount_point` so a display name can differ from the backing resource. |
| `Volume.cloud_config` | generated/types.ts:4633-4635 | `JsonValue \| null` | Service-specific settings (region, endpoint, etc.). |
| `VolumeType = "Cloud"` variant | generated/types.ts:4845-4847 | Part of the `VolumeType` enum | Used for classification / icon routing. |
| `VolumeAddCloudInput` | generated/types.ts:4682 | `{ service: CloudServiceType; display_name: string; config: CloudStorageConfig }` | Wire: `action:volumes.add_cloud.input` (generated/types.ts:4952, 5083). |
| `VolumeAddCloudOutput` | generated/types.ts:4684 | `{ fingerprint: VolumeFingerprint; volume_name: string; service: CloudServiceType }` | |
| `VolumeRemoveCloudInput` | generated/types.ts:4765 | `{ fingerprint: VolumeFingerprint }` | **Exists in the type surface but no UI consumes it** (see §6). |
| `VolumeRemoveCloudOutput` | generated/types.ts:4767 | `{ fingerprint: VolumeFingerprint }` | |
| `GroupType = "Cloud"` variant | generated/types.ts:1717-1720 | Part of the sidebar `GroupType` enum | Exists as a dropdown option, but has no renderer (see §3). |
| `LibraryCreationMethod = "CloudImport"` | generated/types.ts:2347-2350 | Enum variant | Declared, never surfaced in any UI. |
| `VolumeFilter` | generated/types.ts:4712-4724 | `"TrackedOnly" \| "UntrackedOnly" \| "All"` | **No `"CloudOnly"` variant** — frontend cannot list only cloud volumes. |

Not present as a type (searched `CloudCredential`, `OAuthToken`, `DriveKind`, `CloudProvider`): none of these exist. There is no backend concept of stored OAuth credentials exposed to the client — tokens are embedded directly in `CloudStorageConfig`.

---

## 3. Components Inventory

| Component | File | Purpose | Status | Notes |
| --- | --- | --- | --- | --- |
| `AddStorageDialog` / `useAddStorageDialog` | `packages/interface/src/routes/explorer/components/AddStorageModal.tsx` | Multi-step dialog: category → provider → config. Hosts S3/R2/MinIO/B2/Wasabi/Spaces/GDrive/Dropbox/OneDrive/GCS/Azure/pCloud forms. | **PARTIAL** | Calls `volumes.add_cloud` then `locations.add` with `path: { Cloud: {…} }`. OAuth providers ask the user to paste raw `access_token`, `refresh_token`, `client_id`, `client_secret` (AddStorageModal.tsx:1349-1393). No OAuth browser flow, no token exchange. |
| Cloud provider grid | AddStorageModal.tsx:139-212, 828-865 | Provider selection step with 12 cards | FUNCTIONAL | Card grid uses icons from `@sd/assets/icons/Drive-*.png`. |
| `VolumesGroup` | `packages/interface/src/components/SpacesSidebar/VolumesGroup.tsx` | Renders all volumes (local + cloud) in sidebar | FUNCTIONAL (cloud path works) | Uses `getVolumeIcon(volume)` which picks the right icon by parsing `mount_point` scheme (see `volumeIcons.ts:71-87`). Cloud volumes appear alongside local ones, no grouping. |
| `getVolumeIcon` | `packages/ts-client/src/volumeIcons.ts:71-87` | Resolves correct icon based on cloud scheme | FUNCTIONAL | Parses `s3://`, `gdrive://`, `dropbox://`, `onedrive://`, `gcs://`, `azblob://`, `b2://`, `wasabi://`, `spaces://`, `cloud://`. |
| `DevicePanel.getVolumeIcon` | `packages/interface/src/routes/overview/DevicePanel.tsx:55-72` | Duplicate icon resolver for overview | FUNCTIONAL (but limited) | Matches only on name substrings `S3`, `Google`, `Dropbox` — much weaker than `volumeIcons.ts`. |
| `useVolumeContextMenu` | `packages/interface/src/components/SpacesSidebar/hooks/useVolumeContextMenu.ts` | Right-click menu on a volume | **PARTIAL for cloud** | Items: Track / Untrack / Index / Speed Test / Eject. **Does NOT call `volumes.remove_cloud`** — cloud volumes get the generic Untrack flow. Speed Test and Eject are meaningless for cloud. No "Edit credentials", "Reconnect", "Revoke token" items. |
| `Breadcrumb.parseSdPathSegments` | `packages/interface/src/routes/explorer/components/Breadcrumb.tsx:30-44` | Splits Cloud paths into segments for the breadcrumb | FUNCTIONAL | |
| `PathBar` | `packages/interface/src/routes/explorer/components/PathBar.tsx:41-98` | Top navigation bar, handles Cloud paths alongside Physical | FUNCTIONAL | Navigates to parent by slicing `sdPath.Cloud.path`. Comment at line 271: "For Cloud paths, we don't have a device" — handled by skipping device lookup. |
| `FileInspector` cloud handling | `packages/interface/src/components/Inspector/variants/FileInspector.tsx:74-81, 891-892, 1586-1588, 1685` | Renders inspector for files whose `sd_path` is `Cloud` | FUNCTIONAL | Groups cloud instances under `deviceSlug = 'cloud'` (FileInspector.tsx:1586-1588). Cosmetically functional; no cloud-specific affordances. |
| `LocationInspector` | `packages/interface/src/components/Inspector/variants/LocationInspector.tsx:193` | Shows a location's path | FUNCTIONAL | Displays `location.sd_path.Cloud.path` for cloud locations. |
| `FileOperationModal.getFileName` | `packages/interface/src/components/modals/FileOperationModal.tsx:380-398` | Derives filename from `SdPath` | FUNCTIONAL | Handles `Cloud` branch. |
| `AddGroupModal` | `packages/interface/src/components/SpacesSidebar/AddGroupModal.tsx:49, 72` | Lets user add a sidebar group of type `Cloud` | **STUB** | The option is offered in the `<select>` but `SpaceGroup.tsx` has no renderer for `group.group_type === "Cloud"` (see §6) — falls through to the generic Custom/QuickAccess render with no items. |
| `SpaceCustomizationPanel` | `packages/interface/src/components/SpacesSidebar/SpaceCustomizationPanel.tsx:91, 241-242` | Same "Cloud Storage" option in the customization dropdown | **STUB** | Same problem as AddGroupModal. |
| Sidebar "Sources" group | `packages/interface/src/components/SpacesSidebar/SourcesGroup.tsx`, `routes/sources/` | Separate concept: email/notes/bookmarks archive sources | N/A | Not cloud drives. Calls `sources.list`, unrelated to `volumes.add_cloud`. Easy to confuse by name. |
| Tauri `platform.ts` | `apps/tauri/src/platform.ts` | Platform bridge | N/A | **No deep-link / OAuth-callback handler**, no `on_open_url` listener. No custom URL scheme is registered in `apps/tauri/src-tauri/tauri.conf.json`. |
| Mobile `ActionButtons` | `apps/mobile/src/screens/overview/components/ActionButtons.tsx:34-45` | "Setup Sync — Enable cloud backup and sync" | Misleading label | This is P2P/library sync, not cloud-drive connection. No cloud-drive UI on mobile. |

Duplicate logic note: `getVolumeIcon` is defined twice (`packages/ts-client/src/volumeIcons.ts:71` and `packages/interface/src/routes/overview/DevicePanel.tsx:55`). The DevicePanel copy is weaker (name-based) and disagrees with the ts-client copy (scheme-based).

---

## 4. Routes / Pages Inventory

| Route | File | Purpose | Status | Notes |
| --- | --- | --- | --- | --- |
| `/` (overview) | `packages/interface/src/routes/overview/` | Devices + volumes overview | FUNCTIONAL (includes cloud volumes) | `OverviewTopBar.tsx:242-253, 302-309` hosts the "Add Storage" button that opens `AddStorageModal`. The `CloudArrowUp` icon at line 10/217 is for the "Setup Sync" (P2P) action, not cloud. |
| `/explorer` | `packages/interface/src/routes/explorer/` | File browser | FUNCTIONAL for cloud paths | Accepts `sd_path` including `Cloud` variant. |
| `/sources`, `/sources/adapters`, `/sources/:id` | `packages/interface/src/routes/sources/` | Archive data sources (email/etc.) | N/A | Unrelated to cloud drives. |
| `/settings` | `packages/interface/src/routes/settings/` → `Settings/pages/` | Library/app settings | **No cloud page** | Pages: About, Advanced, Appearance, General, Indexer, Library, Privacy, Services. No "Connected Accounts", no "Cloud Storage", no "OAuth tokens". |
| Onboarding | — | — | **MISSING** | No onboarding route or component anywhere in `packages/interface/src` (grep for `Onboarding\|onboard\|welcome` returns no component matches). |
| OAuth callback route | — | — | **MISSING** | No `/oauth/callback`, no `spacedrive://` scheme, no deep-link handler in Tauri. |

---

## 5. OAuth / Auth UX

**Current state: manual-only.**

For Google Drive, OneDrive, Dropbox (AddStorageModal.tsx:1349-1394):

```
isOAuthType && (
  <Input {...register("client_id")} />
  <Input {...register("client_secret")} type="password" />
  <Input {...register("access_token")} />
  <Input {...register("refresh_token")} />
  <Input {...register("root")} />   // optional
)
```

The user is expected to:
1. Register their own OAuth app with Google/Microsoft/Dropbox.
2. Obtain `client_id` and `client_secret`.
3. Run an external OAuth dance (not provided) to exchange a code for an `access_token` + `refresh_token`.
4. Paste all four values into the dialog.

**What is missing to make this consumer-usable:**

- **No custom URL scheme registration.** `apps/tauri/src-tauri/tauri.conf.json` has no `deepLinks` / `protocol` configuration. Grep for `deep.?link|deepLink|scheme|oauth|callback` in `apps/tauri/src-tauri` returns zero OAuth hits.
- **No `plugin-deep-link` or `plugin-oauth` dependency.** Not found in `apps/tauri/package.json` context (only `@tauri-apps/plugin-dialog`, `plugin-shell` are imported in `platform.ts:1-5`).
- **No redirect URI handler.** `platform.ts` has no `on_open_url` / deep-link listener.
- **No embedded webview / popup flow.** `platform.openLink(url)` (platform.ts:46-48) opens in the external browser, but nothing listens for the redirect back.
- **No PKCE code generator, no state token, no code verifier** in the TS code.
- **No shared/public client credentials.** Spacedrive does not ship OAuth app credentials, so each user has to register their own Google Cloud project / Azure app / Dropbox app.
- **No token refresh UI.** Access tokens expire — there is no way to re-enter or re-authorize from inside the app. User would need to remove the volume and re-add it.

The backend contract already assumes the frontend will supply valid `access_token + refresh_token`, so the gap is strictly a frontend one (types.ts:186-202).

---

## 6. Integration Points

### Where cloud IS surfaced

1. **"Add Storage" button** in the Overview top bar (OverviewTopBar.tsx:242-253) — opens the category chooser, where "Cloud Storage" is one of four equal-weight options (AddStorageModal.tsx:119-124).
2. **Explorer PathBar "+"** — same dialog (PathBar.tsx:22).
3. **Sidebar volumes list** — cloud volumes appear automatically with provider icons (VolumesGroup.tsx + volumeIcons.ts).
4. **Breadcrumb, FileInspector, LocationInspector, FileOperationModal** — all handle `'Cloud' in sdPath` and render cloud paths correctly.

### Where cloud SHOULD be surfaced but ISN'T

1. **Settings → "Connected Accounts" page** — does not exist. There is no way to see which cloud accounts you've connected without inspecting the sidebar.
2. **Library creation** (`CreateLibraryModal.tsx`) — the `LibraryCreationMethod = "CloudImport"` enum variant exists (types.ts:2350) but there is no "Import from cloud" button in the library creation dialog.
3. **Onboarding** — no onboarding exists at all, let alone a "connect your cloud drives" step.
4. **Dedicated Cloud sidebar group** — `GroupType = "Cloud"` is offered in `AddGroupModal` (AddGroupModal.tsx:49) and `SpaceCustomizationPanel` (line 241-242), but `SpaceGroup.tsx:65-132` has no branch for `group_type === "Cloud"` — it falls through to the generic Custom renderer with zero items. This is a broken dropdown option.
5. **Volume filter** — `VolumeFilter` has `TrackedOnly | UntrackedOnly | All` but no `CloudOnly`, so the frontend can't easily show "just my cloud volumes".
6. **Volume context menu — no cloud-specific actions.** `useVolumeContextMenu.ts` has Track / Untrack / Index / Speed Test / Eject. It should have Reconnect / Re-authenticate / Edit credentials / Remove cloud volume (`volumes.remove_cloud` is wired in types but never called from TS).
7. **Empty state of the Volumes sidebar** — VolumesGroup.tsx:100-103 shows "No volumes", with no CTA to add one.

---

## 7. Placeholder / TODO inventory (cloud-adjacent)

| File:Line | Marker | Text |
| --- | --- | --- |
| `AddStorageModal.tsx:883` | `<strong>Coming Soon</strong>` | For network protocols (SMB/NFS/SFTP/WebDAV), not cloud — but disables the network category entirely. WebDAV should arguably be reachable via cloud. |
| `AddStorageModal.tsx:401` | `console.log("AddStorageDialog render:", …)` | Debug console.log shipped to production. |
| `AddStorageModal.tsx:576, 584, 748, 772` | `console.log` / `console.error` | More debug logging. |

No `TODO` / `FIXME` comments in cloud-specific code paths. The cloud surface is not explicitly marked as incomplete — it is simply missing the OAuth UX and the management UI.

No i18n / translation files exist in the interface package (grep for `i18n|translation|locale` in `packages/interface/src` returns only unrelated hits and state setters). All cloud copy is hard-coded English in AddStorageModal.tsx.

---

## 8. Critical gaps for MVP UX

Ranked by user impact:

1. **Interactive OAuth flow for GDrive / Dropbox / OneDrive.** Today the user must paste four raw tokens. For MVP this needs: (a) shipped public OAuth client IDs in the daemon, (b) a `spacedrive://oauth/callback` deep link registered in `tauri.conf.json`, (c) a listener in `apps/tauri/src/platform.ts` and a corresponding `core` endpoint, (d) a "Connect" button that opens the provider's consent screen via `platform.openLink()`, (e) token exchange on the backend. The current form fields (`access_token`, `refresh_token`) should collapse to a single "Connect" button.

2. **Connected Accounts settings page.** A `Settings/pages/CloudSettings.tsx` listing every tracked cloud volume, showing service + bucket/drive, last sync, and buttons to Reconnect / Disconnect / Remove. Must call `volumes.remove_cloud` (currently dead code from the TS side).

3. **Cloud-aware context menu on cloud volumes.** Branch `useVolumeContextMenu.ts` so Speed Test / Eject are hidden and Reconnect / Edit credentials / Remove are shown for volumes where `volume.cloud_identifier !== null`.

4. **Dedicated Cloud sidebar group renderer.** Either implement `group.group_type === "Cloud"` in `SpaceGroup.tsx` (filtering volumes where `cloud_identifier !== null`), or remove the option from `AddGroupModal.tsx:49` and `SpaceCustomizationPanel.tsx:241`. Today it is a broken dropdown entry.

5. **Backend-supplied `VolumeFilter = "CloudOnly"`** to make §4 efficient without client-side filtering.

6. **Single source of truth for cloud icon resolution.** Delete `DevicePanel.getVolumeIcon` (DevicePanel.tsx:55) and use `getVolumeIcon` from `@sd/ts-client` (volumeIcons.ts:71) everywhere. The DevicePanel version is strictly weaker and will mis-icon cloud volumes whose name does not contain "S3"/"Google"/"Dropbox".

7. **OAuth error UI.** Cloud mutations currently surface errors via `console.error` + a red `<p>` inside the dialog (AddStorageModal.tsx:748-755). Expired-token errors from the daemon need a distinct "Reconnect" CTA, not a generic error string.

8. **Add "Import from Cloud" in `CreateLibraryModal`** to fulfill the `LibraryCreationMethod = "CloudImport"` enum contract (types.ts:2350).

9. **Remove debug `console.log` from AddStorageModal.tsx:401** before any user-facing release.

10. **Strip dead asset imports** — `AddStorageModal.tsx:48-49` imports `DriveDAV` and `DriveBox` which are only used in disabled "Coming Soon" cards and unrelated network/Azure sections; Mega/OpenStack icons exist in `packages/assets` but no provider uses them. Either expose them as provider options or delete the assets.

