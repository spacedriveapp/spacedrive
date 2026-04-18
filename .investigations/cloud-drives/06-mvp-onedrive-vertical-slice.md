# MVP Cloud Drives — OneDrive Vertical Slice (BYO)

**Date:** 2026-04-18
**Branche:** `feature/cloud-drives-investigation`
**Statut:** Plan d'exécution (aucun code modifié)
**Superseeds:** `05-implementation-plan.md` comme plan opérationnel. Le doc 05 reste la référence stratégique multi-provider.

---

## 1. Scope décidé

### Architecture : BYO (Bring Your Own client_id)

**Contexte** : le code Spacedrive actuel exige déjà `client_id + client_secret` de l'utilisateur pour chaque provider OAuth (`core/src/ops/volumes/add_cloud/action.rs:168-175` valide qu'ils sont non-vides). Aucun design doc n'a jamais planifié que Spacedrive possède ses propres apps OAuth enregistrées, et c'est un choix cohérent avec l'ethos local-first du projet (moins de dépendance à Spacedrive en tant qu'entité).

Le MVP **garde** cette architecture BYO mais en **corrige radicalement l'UX** :

- Aujourd'hui : l'utilisateur doit obtenir manuellement `access_token` + `refresh_token` via curl/Postman → developer-only, aucun moyen de le faire marcher pour un utilisateur normal.
- Après MVP : l'utilisateur colle UNE seule fois son `client_id` + `client_secret` (avec un tutoriel intégré qui l'aide à les obtenir), puis clique **un bouton "Connect with Microsoft"** qui lance un flow browser OAuth standard. Les tokens sont obtenus automatiquement.

### Inclus
- **Provider unique** : Microsoft OneDrive personnel (`tenant=common`, comptes consumer)
- **Parcours utilisateur end-to-end BYO** :
  1. User ouvre le modal "Add Cloud Storage → OneDrive"
  2. Si pas déjà fait, user suit le **tutoriel Azure AD intégré** pour créer sa propre app (screenshots + copy-to-clipboard des redirect URIs + liens directs au portail Entra ID)
  3. User colle son `client_id` + `client_secret` dans les champs du modal
  4. User clique **"Connect with Microsoft"** → browser s'ouvre → login MSFT → consent → redirect loopback → tokens récupérés
  5. Volume créé + apparaît dans la sidebar
  6. Browse / read / write / copy / restart / disconnect comme dans n'importe quel volume
- **Basic change detection** via `/drive/root/delta` pour ne pas ré-hasher à chaque indexation
- **Fix problèmes bloquants** transverses (panics FileCopyJob, `create_folder` déconnecté, bug UI `GroupType::Cloud`, `getVolumeIcon` dupliqué, `volumes.remove_cloud` pas câblé)
- **Pas de modification du schéma DB** lié aux credentials (on garde `CloudStorageConfig::OneDrive` tel quel)
- **Cherry-pick production-ready** sans toucher plus que nécessaire à l'UI

### Exclus (TODO explicites dans le code, cf §9)
- Google Drive et Dropbox OAuth browser flow (infra OAuth réutilisable, provider à écrire)
- OneDrive Business / SharePoint / Work accounts (tenant spécifique)
- Spacedrive-owned apps publiques (si la decision est prise plus tard, c'est un diff de ~5 lignes : remplacer les inputs par des constantes)
- Settings page "Connected Accounts" (right-click menu fait office)
- Initial sync progress UX fine
- Cloud-specific sidebar group (`GroupType::Cloud` renderer)
- S3/GCS/Azure change detection évoluée
- Dropbox longpoll
- Pré-remplissage du portail Azure via URL magique (approche C de notre discussion — Phase 2)
- Observability / métriques fines

### Différences vs `05-implementation-plan.md`
| Aspect | Plan 05 (multi-provider) | Plan 06 (OneDrive MVP, BYO) |
|---|---|---|
| Providers | 3 OAuth + 6 object stores | 1 (OneDrive) |
| OAuth impl | Générique 3 providers + public client | Générique + 1 provider concret, **BYO-based** |
| Change detection | 3 stratégies | Stratégie Delta uniquement (OneDrive) |
| Durée estimée | 10-12 semaines | 3-4 semaines |
| UI | Settings "Connected Accounts" + reconnect UI | Tutoriel BYO + "Connect" button + right-click disconnect |
| Dépendance externe | Azure AD app à créer par Spacedrive | **Aucune** |
| Domaine modifié | Nouveau `CloudAccount` | **Aucun nouveau domaine** |

---

## 2. Principes opérationnels (règles de session)

Ces règles s'appliquent à chaque PR de ce MVP. Prises comme engagement explicite avec le founder :

1. **Root cause, pas symptôme.** Si on trouve un bug, on corrige la cause, pas son effet.
2. **Respecter les patterns du projet.** CQRS + DDD (`src/domain/` nouns, `src/ops/` verbs), `register_library_action!` pour les actions, `thiserror` pour les erreurs typées, `tracing` (jamais `println!`), `useLibraryMutation` côté TS (jamais `fetch` manuel), classes Tailwind sémantiques (`bg-sidebar` pas `bg-[var(...)]`).
3. **Zéro dette tech laissée derrière.** On fixe les problèmes qu'on rencontre même si non-bloquants — sauf si vraiment hors scope, auquel cas `TODO(cloud-mvp): <description précise> — see .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#N`.
4. **Verify before act.** Toute affirmation d'un agent, rapport, ou mémoire est vérifiée contre le code réel avant d'agir (règle AGENTS.md globale — ~35 % des findings d'agents sont faux).
5. **PR petits et indépendants.** Cible ≤ 400 lignes diff par PR. Chaque PR passe CI verte seul. Compilable + testable entre chaque merge.
6. **Types backend-first.** Chaque type public (`#[derive(Type)]`) suit `cargo run --bin generate_typescript_types`. Jamais de type TS manuel qui duplique un type Rust. Jamais de `as any`.
7. **Async-clean.** Pas de blocage du runtime : `tokio::fs` pas `std::fs`, `tokio::sync::RwLock` pas `std::sync::RwLock`, `spawn_blocking` pour le CPU-lourd.
8. **Pas d'emojis** dans le code (règle AGENTS.md).

### Task tracking
- Créer/mettre à jour les tasks `.tasks/core/CLOUD-003-cloud-volume.md` et `FILE-003-cloud-volume-file-operations.md` au fur et à mesure
- Chaque PR met à jour le checkbox correspondant dans la task
- Statut `In Progress` → `Done` au merge (via `cargo run --bin task-validator -- validate`)

---

## 3. User journey (critères d'acceptation du MVP)

### Phase A — Setup (one-time par compte)
1. User ouvre Spacedrive
2. Sidebar → "Add Cloud Storage" → sélectionne OneDrive
3. Modal s'ouvre avec deux sections :
   - **Section tutoriel** (collapsible, auto-expanded la première fois) : guide visuel étape par étape pour créer une app Azure AD perso :
     - "Go to https://portal.azure.com → Entra ID → App registrations → New registration" avec screenshot de référence
     - "Name: Spacedrive (or anything) / Account types: Personal Microsoft accounts only"
     - "Add a redirect URI of type Public client/native" + bouton "Copy redirect URIs" qui copie les 5 ports loopback
     - "Authentication → Advanced settings → Allow public client flows: Yes"
     - "API permissions → Add → Microsoft Graph → Files.ReadWrite.All, offline_access, User.Read"
     - "Overview → copy Application (client) ID" avec screenshot
     - "Certificates & secrets → New client secret → copy value (attention, only visible once)"
   - **Section input** : 2 champs — `Application (client) ID` et `Client secret`
4. User colle les 2 valeurs, clique **"Connect with Microsoft"**
5. Browser système s'ouvre sur login.microsoftonline.com
6. User s'authentifie et accepte les scopes
7. Browser redirige vers `http://127.0.0.1:PORT/oauth/callback`
8. Page "You can close this tab" s'affiche
9. Modal affiche "OneDrive connected as <user display_name>"

### Phase B — Volume visible et utilisable
10. Le volume OneDrive apparaît dans la sidebar sous **Volumes** (groupe existant), avec l'icône OneDrive correcte
11. Click → browse racine OneDrive → voir fichiers et dossiers
12. Navigation dans sous-dossiers fonctionne (Breadcrumb, PathBar)
13. Metadata sur chaque entrée (taille, last modified) correcte

### Phase C — Indexation incrémentale
14. Première visite : indexation du dossier courant (progress bar existant)
15. Seconde visite : seules les modifications sont traitées via Delta API
16. Renommage côté Microsoft → tags/notes Spacedrive préservés (car `provider_file_id` stable)

### Phase D — Opérations
17. Double-click fichier → download à la demande → preview/app externe
18. Streaming : fichier 2 GB ne charge pas tout en mémoire
19. Drag local → OneDrive folder : upload streaming
20. Drag OneDrive → local folder : download
21. Aucun `panic!` dans le daemon pendant ces opérations
22. Right-click dans OneDrive → "New Folder" → dossier créé côté Microsoft

### Phase E — Restart
23. Quit Spacedrive, redémarre
24. Volume OneDrive toujours présent
25. Re-auth silencieuse via refresh_token (background task)
26. Navigation reprend sans action utilisateur

### Phase F — Déconnexion
27. Right-click sur volume OneDrive → "Disconnect"
28. Confirmation modal
29. `volumes.remove_cloud` appelé (pas `volumes.untrack` générique)
30. Credentials encryptés supprimés de la DB
31. Volume disparaît de la sidebar

### Gates de qualité
- Chaque étape sans crash, sans panic, sans toast d'erreur non-expliqué
- Logs `tracing` clairs avec `volume_id` structuré (note : pas `account_id` — plus de concept `CloudAccount` pour MVP)
- Tests d'intégration couvrent au minimum étapes 4-8, 15, 19-20, 22, 24-25, 27-30

---

## 4. Découpage en PRs

Chaque PR compile seul, passe `cargo test`, passe `cargo clippy`, passe `cargo fmt --check`. Titre préfixe `cloud-mvp:`.

### PR 1 — Foundation (OpenDAL 0.55 + Layers + BackendFeatures)
**Objectif :** base saine avant tout feature work. Aucun changement visible utilisateur.

Périmètre :
- [ ] Bump `opendal = "0.54"` → `"0.55"` dans `core/Cargo.toml`
- [ ] Adapter les 2 sites `jiff::Timestamp` dans `core/src/volume/backend/cloud.rs:334, :361`
- [ ] Dans `CloudBackend::new_*`, ajouter stack `RetryLayer::default().with_max_times(3)` + `TimeoutLayer::new(Duration::from_secs(30))` + `ConcurrentLimitLayer::new(16)` + `TracingLayer`
- [ ] Nouveau struct `BackendFeatures` dans `core/src/volume/backend/mod.rs` avec `server_side_copy`, `server_side_rename`, `stable_file_id`, `change_notifications: ChangeNotificationKind`, `content_hash: Option<HashAlgorithm>`, `case_insensitive`, `supports_duplicates`, `max_file_size`, `multipart_threshold`
- [ ] Méthode `VolumeBackend::features(&self) -> &BackendFeatures`. Valeurs correctes pour `LocalBackend` et `CloudBackend` (se base sur `CloudServiceType` + `Operator::info().full_capability()`)
- [ ] Enrichir `BackendMetadata` (trait) avec `etag: Option<String>`, `version: Option<String>`, `content_md5: Option<String>`, `provider_file_id: Option<String>`
- [ ] Fix bug `CloudBackend::exists()` qui swallow les erreurs d'auth en `false` : distinguer `ErrorKind::NotFound` (→ `false`) du reste (→ `Err`)
- [ ] Unit tests : round-trip d'un blob via `services::Memory` exerçant tous les nouveaux layers
- [ ] Generate TypeScript types (`cargo run --bin generate_typescript_types`)

**Fichiers touchés :** `core/Cargo.toml`, `core/src/volume/backend/mod.rs`, `core/src/volume/backend/cloud.rs`, `core/src/volume/backend/local.rs`, `packages/ts-client/src/generated/types.ts`.

**Out of scope (TODO):** `BackendFeatures` n'est utilisé par aucun consumer dans cette PR. Les consumers viennent en PR 2+.

---

### PR 2 — Fix les panics + wire create_folder
**Objectif :** supprimer les 2 blocants hard-crash et reconnecter une feature triviale déjà implémentée.

Périmètre :
- [ ] `core/src/ops/files/copy/job.rs:1042, :1485` : remplacer `panic!` par logique conditionnelle :
  - Si `src` et `dst` sur **même backend cloud** avec `features.server_side_copy == true` : utiliser `CloudBackend::copy()` (nouvelle méthode qui appelle `operator.copy()`)
  - Sinon : stream via `reader()` → `writer_with().chunk(8 * MiB).concurrent(4)` (limite mémoire ~32 MiB par copie)
  - Jamais `panic!` : toute erreur devient `Err(FileCopyError::Cloud(#[from] ...))`
- [ ] Nouvelle méthode `CloudBackend::copy(src: &str, dst: &str) -> Result<()>` gated sur `Capability::write_can_copy`
- [ ] `core/src/ops/files/create_folder/action.rs:120` : replace `ActionError::Internal("Cloud folder creation not yet implemented")` par appel à `CloudBackend::create_directory()` qui existe déjà (`cloud.rs:412-432`)
- [ ] Extension `thiserror` dans `FileCopyError` pour les nouvelles branches cloud
- [ ] Unit tests : 3 scénarios (cloud→cloud server-side, cloud→cloud streaming fallback, local↔cloud) via `services::Memory`
- [ ] Integration test : `CreateFolderAction` sur `services::Memory` backend

**Fichiers touchés :** `core/src/ops/files/copy/{job,error}.rs`, `core/src/ops/files/create_folder/action.rs`, `core/src/volume/backend/cloud.rs`, `core/src/volume/backend/mod.rs`.

**Out of scope (TODO):** cross-backend server-side copy (S3 → S3 cross-bucket) — Phase 2.

**Tasks impactées :** `.tasks/core/FILE-003-cloud-volume-file-operations.md` passe "To Do" → "In Progress" puis "Done" au merge.

---

### PR 3 — OAuth infrastructure (générique, provider-agnostic, BYO-compatible)
**Objectif :** plomberie OAuth RFC 8252 + PKCE + loopback. **Aucun `CloudAccount` domain** — les credentials restent attachés aux volumes comme aujourd'hui.

Périmètre :
- [ ] Deps : `oauth2 = "5"` + `webbrowser = "1"` dans `core/Cargo.toml`
- [ ] Nouveau module `core/src/ops/cloud/oauth/`, structure CQRS :
  - `action.rs` : `CloudOauthStartAction` (registered `cloud.oauth.start`), `CloudOauthCompleteAction` (registered `cloud.oauth.complete`), `CloudOauthCancelAction` (registered `cloud.oauth.cancel`)
  - `query.rs` : `CloudOauthPollQuery` (registered `cloud.oauth.poll`) — renvoie status du flow
  - `input.rs` / `output.rs` / `error.rs` : types clean avec `thiserror`
  - `flow.rs` : struct `OauthFlow { flow_id, provider, pkce_verifier, state, loopback_port, client_id, client_secret }` (in-memory, `Arc<DashMap<Uuid, OauthFlow>>` sur `CoreContext` — purgé après 10 min)
  - `server.rs` : `run_loopback_callback_server(port, state) -> Result<AuthCode>` — tokio listener, `hyper` handler, timeout 5min, single-shot, valide `state` pour CSRF
  - `providers/mod.rs` : trait `OauthProvider { fn auth_url(&self, flow: &OauthFlow) -> Url; fn exchange_code(&self, code: &str, verifier: &str, client_id: &str, client_secret: &str) -> Result<TokenSet>; fn refresh(&self, refresh_token: &str, client_id: &str, client_secret: &str) -> Result<TokenSet>; }`
  - Note : pas de `OauthProviderConfig` stocké — le `client_id`/`client_secret` arrivent en input utilisateur à chaque flow
- [ ] `CloudOauthStartInput { provider, client_id, client_secret }` → `CloudOauthStartOutput { flow_id, auth_url }` — l'UI ouvre `auth_url` via `webbrowser::open`
- [ ] `CloudOauthPollQuery { flow_id }` → `CloudOauthPollOutput { status: Pending | Completed { access_token, refresh_token, expires_at, display_name } | Failed { error } | Cancelled }`
- [ ] Background task `CloudTokenRefreshTask` lancé au startup — scan `cloud_credentials` table, rafraîchit les tokens 5min avant expiry en appelant `OauthProvider::refresh` avec les `client_id`/`client_secret` **déjà stockés** dans `CredentialData::OAuth`
- [ ] Sur échec refresh → log warning + marquage status du volume (extension de `VolumeStatus` ? ou colonne `needs_reauth` sur `cloud_credentials` ?) — à décider en PR 3 selon ce qui existe déjà
- [ ] Unit tests : flow state machine, state validation (CSRF), PKCE generation, mock provider exchange

**Fichiers touchés :** `core/Cargo.toml`, tout `core/src/ops/cloud/oauth/`, `core/src/bootstrap/` (task registration), potentiellement `core/src/crypto/cloud_credentials.rs` si ajout d'un champ status.

**Out of scope (TODO):** device-code flow pour SSH, `CloudAccount` domain (Phase 2 quand plusieurs volumes partagent le même token).

---

### PR 4 — OneDrive OAuth provider + augment `volumes.add_cloud`
**Objectif :** brancher OneDrive sur l'infra de la PR 3. Pas de nouveau type `CloudStorageConfig::OneDriveOauth` — on augmente la branche existante.

Périmètre :
- [ ] `core/src/ops/cloud/oauth/providers/onedrive.rs` : impl `OauthProvider` pour OneDrive
  - Issuer : `https://login.microsoftonline.com/common/oauth2/v2.0`
  - Auth endpoint : `.../authorize`
  - Token endpoint : `.../token`
  - Scopes : `Files.ReadWrite.All offline_access User.Read` (User.Read pour récupérer display_name après exchange)
  - Redirect URI : `http://127.0.0.1:{port}/oauth/callback` — port choisi dynamiquement dans `[53682, 53683, 53684, 53685, 53686]` (Microsoft supporte 127.0.0.1 en loopback exact-match seulement sur ports enregistrés)
  - PKCE S256
- [ ] Après `exchange_code` réussi : appel `https://graph.microsoft.com/v1.0/me` pour récupérer `displayName` et `userPrincipalName` → retournés dans `CloudOauthPollOutput.Completed.display_name`
- [ ] **Le flow utilisateur est** :
  1. UI appelle `cloud.oauth.start { provider: OneDrive, client_id, client_secret }` → reçoit `{ flow_id, auth_url }`
  2. UI ouvre `auth_url` via webbrowser::open
  3. UI poll `cloud.oauth.poll { flow_id }` chaque seconde
  4. Quand `status: Completed { access_token, refresh_token, expires_at, display_name }`, UI appelle directement `volumes.add_cloud { service: OneDrive, display_name, config: OneDrive { access_token, refresh_token, client_id, client_secret, root: None } }`
  5. `volumes.add_cloud` (inchangé) valide + crée le volume + persiste les credentials encryptés
- [ ] **Aucune modification de `CloudStorageConfig::OneDrive`** ni de `VolumeAddCloudAction`. La backward-compat est totale : le paste-tokens flow continue de marcher exactement comme avant (Q-MVP-3 supprimée — pas de retrocompat à gérer)
- [ ] Tests :
  - Mock server HTTP (wiremock) simulant Microsoft OAuth endpoints + Graph /me
  - Test full flow : start → callback → exchange → tokens + display_name retournés
  - Test refresh sur token expiré
  - Test rejection sur state mismatch (CSRF)
  - Test timeout après 5min sans callback

**Fichiers touchés :** `core/src/ops/cloud/oauth/providers/{mod,onedrive}.rs`.

**Out of scope (TODO):**
- Scopes Business/SharePoint (nécessite tenant id dynamique). Branch dans `onedrive.rs` avec `TODO(cloud-mvp): Extend OAuth to Microsoft Work/Business accounts — currently personal only via tenant=common`
- Google Drive / Dropbox providers — infra prête, provider à écrire : `TODO(cloud-mvp): Implement GoogleDriveOauthProvider using the same trait`

---

### PR 5 — Frontend : modal OneDrive avec tutoriel BYO + bouton Connect
**Objectif :** UX OneDrive end-to-end : tutoriel clair + bouton browser.

Périmètre :
- [ ] `packages/interface/src/routes/explorer/components/AddStorageModal.tsx` : remplacer la section OneDrive (actuellement lignes 1349-1394 avec 4 inputs en vrac) par un composant dédié `OneDriveConnectForm.tsx` avec :
  - **Section tutoriel (collapsible)** : `<details>` natif ou composant custom `<Disclosure>` (Radix) avec :
    - 7 étapes numérotées, chaque étape a : titre, description courte, éventuellement un screenshot (à stocker dans `packages/assets/tutorials/onedrive/step-N.png`)
    - Bouton "Open Azure Portal" qui ouvre `https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade` via `openExternal`
    - Bouton "Copy redirect URIs" qui copie `http://127.0.0.1:53682/oauth/callback\nhttp://127.0.0.1:53683/oauth/callback\n...` (5 URIs separated par `\n`)
  - **Section input (toujours visible)** :
    - 2 champs : `clientId` (text) + `clientSecret` (password)
    - Validation côté client : non-vides, `clientId` est un UUID v4 pattern
  - **Bouton "Connect with Microsoft"** :
    - Disabled tant que les 2 inputs sont vides/invalides
    - Au click : `useLibraryMutation('cloud.oauth.start')` avec `{ provider: 'OneDrive', client_id, client_secret }` → reçoit `{ flow_id, auth_url }`
    - `openExternal(auth_url)`, puis `setFlowId(flow_id)` qui déclenche le polling
  - **Polling state** :
    - `useLibraryQuery({ type: 'cloud.oauth.poll', input: { flow_id } }, { enabled: !!flowId, refetchInterval: 1000 })`
    - Sur `status.Completed { access_token, refresh_token, display_name }` :
      - Appelle `useLibraryMutation('volumes.add_cloud', { service: 'OneDrive', display_name, config: { OneDrive: { access_token, refresh_token, client_id, client_secret, root: null } } })`
      - Ferme le modal, toast succès
    - Sur `status.Failed { error }` : toast d'erreur avec message du backend
    - Bouton "Cancel" → `useLibraryMutation('cloud.oauth.cancel', { flow_id })` + clear state
- [ ] Utiliser hooks type-safe `useLibraryMutation`, `useLibraryQuery` — jamais `fetch` manuel
- [ ] Classes Tailwind sémantiques uniquement (`bg-accent`, `text-ink`, `rounded-lg`, `bg-app-input`, etc.)
- [ ] Effects uniquement pour le polling (sync externe). State du polling dérivé via `useLibraryQuery`, pas via `useEffect + setState`
- [ ] Les **autres providers OAuth** (Gdrive, Dropbox) **gardent** leur paste-tokens UI actuelle avec un petit banner « Browser sign-in coming soon for Google Drive and Dropbox — for now, please paste tokens manually »

**Fichiers touchés :** `packages/interface/src/routes/explorer/components/AddStorageModal.tsx` (modification ciblée pour OneDrive), nouveau `packages/interface/src/routes/explorer/components/OneDriveConnectForm.tsx`, éventuellement nouveaux assets dans `packages/assets/tutorials/onedrive/`.

**Out of scope (TODO):**
- Pré-remplissage Azure portal via URL magique (Phase 2, approche C de la discussion) : `TODO(cloud-mvp): Pre-fill Azure AD app registration via portal URL params to reduce tutorial friction`
- Tutoriels équivalents pour Gdrive/Dropbox : viendront avec leurs provider PRs respectives

---

### PR 6 — Delta API change detection pour OneDrive
**Objectif :** ré-indexer intelligemment au lieu de re-hasher toute la librairie.

Périmètre :
- [ ] Nouvelle table `cloud_sync_state` via migration : `(volume_id PK/FK, provider, change_token TEXT, last_full_sync_at, last_incremental_at, consecutive_failures INT)`
- [ ] Entity SeaORM + trait `CloudSyncStateRepository`
- [ ] Nouveau trait dans `core/src/volume/backend/mod.rs` :
  ```rust
  #[async_trait]
  pub trait ChangeDetector: Send + Sync {
      async fn initial_token(&self) -> Result<ChangeToken>;
      async fn changes_since(&self, token: &ChangeToken) -> Result<ChangesPage>;
  }
  ```
  Avec `ChangesPage { entries: Vec<ChangeEntry>, next_token: Option<ChangeToken>, end_token: Option<ChangeToken> }` — différencier pagination intra-scan vs end-of-changes
- [ ] Impl `OneDriveChangeDetector` qui appelle directement Microsoft Graph `/drive/root/delta` (hors OpenDAL — OpenDAL ne l'expose pas, voir research/02 §5). Utilise l'`access_token` stocké dans `cloud_credentials`.
- [ ] Gestion 410 Gone + `resyncRequired` → retourne `Err(ChangeDetectionError::Invalidated)` → caller déclenche full resync
- [ ] Réécriture ciblée de `core/src/ops/indexing/phases/processing.rs:270-288` : pour un volume cloud avec `ChangeDetector`, utilise `cloud_sync_state` ; sinon fallback au comportement actuel (S3 family, etc.)
- [ ] Fix `core/src/ops/indexing/path_resolver.rs:227` : résoudre cloud path via `provider_file_id` stocké sur `entries` (colonne à ajouter dans une micro-migration, nullable)
- [ ] Checkpoint du `change_token` après **chaque page réussie** (pas fin de scan) — research/03 §6
- [ ] Integration tests : mock Graph avec un trace enregistré, vérifier qu'une 2e indexation ne re-hashe que les fichiers modifiés, rename préserve `provider_file_id`
- [ ] Scheduler : poll 60s foreground / 5min background, backoff exponentiel 429/503

**Fichiers touchés :** `core/src/infra/db/migration/`, `core/src/infra/db/entities/cloud_sync_state.rs`, `core/src/volume/backend/{mod,cloud}.rs`, nouveau `core/src/ops/cloud/change_detection/onedrive.rs`, `core/src/ops/indexing/phases/processing.rs`, `core/src/ops/indexing/path_resolver.rs`.

**Out of scope (TODO):**
- Change detection pour les autres providers — `TODO(cloud-mvp): Implement GoogleDriveChangeDetector using changes.list API`
- `TODO(cloud-mvp): Implement DropboxChangeDetector using list_folder/continue + longpoll`
- `TODO(cloud-mvp): LIST-diff strategy for S3/GCS/Azure without delta API`

---

### PR 7 — Fixes annexes (right-click disconnect + GroupType + icons)
**Objectif :** nettoyer la dette identifiée dans les rapports 03, sans élargir le scope UI.

Périmètre :
- [ ] `packages/interface/src/components/SpacesSidebar/hooks/useVolumeContextMenu.ts` : pour `volume.backend_type.Cloud`, ajouter item "Disconnect" qui appelle `useLibraryMutation('volumes.remove_cloud')` avec confirm modal. Retirer "Speed Test" et "Eject" pour les cloud volumes (ne font pas sens). "Untrack" reste disponible.
- [ ] Fix bug `GroupType::Cloud` :
  - Retirer l'option `<option value="Cloud">Cloud Storage</option>` de `AddGroupModal.tsx:49`
  - Retirer l'option équivalente de `SpaceCustomizationPanel.tsx:241-242`
  - **Garder la variante** `Cloud` dans `core/src/domain/space.rs:237-238` (retirer casserait la déserialisation des rows JSON existants) avec doc-comment mis à jour :
  ```rust
  /// Cloud storage providers
  ///
  /// TODO(cloud-mvp): Implement CloudGroup renderer analogous to VolumesGroup/DevicesGroup.
  /// Temporarily hidden from AddGroupModal / SpaceCustomizationPanel dropdowns until implemented.
  /// See `.investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#pr-7`.
  Cloud,
  ```
  - Ajouter commentaire TS dans les deux modals pointant vers l'enum côté Rust
- [ ] Unifier les 2 `getVolumeIcon` : supprimer la version affaiblie dans `routes/overview/DevicePanel.tsx:55-72`, importer la canonique `packages/ts-client/src/volumeIcons.ts:71`
- [ ] Unit test frontend : `getVolumeIcon` renvoie la bonne icône pour un volume OneDrive renommé ("Mon espace cloud")

**Fichiers touchés :** 4 TS, 1 RS.

**Out of scope :** rien ici — tout est borné et propre.

---

### PR 8 — Tests + docs sync
**Objectif :** durcir et aligner la documentation sur la réalité.

Périmètre :
- [ ] Test d'intégration `core/tests/onedrive_end_to_end_test.rs` :
  - Mock Graph server (via `wiremock`) — login.microsoftonline.com + graph.microsoft.com
  - Scenario complet : OAuth flow → add volume → list root → read file → write file → rename (via delta) → delete → remove_cloud
  - `#[tokio::test]`, pas de `#[ignore]`
- [ ] Test restart : add OneDrive volume, shutdown library manager, re-load, verify volume rehydrates + tokens refresh
- [ ] Réécrire `docs/core/cloud-integration.mdx` pour refléter la réalité :
  - Supprimer la liste "40+ providers" aspirationnelle (remplacer par la liste des 9 providers réellement wired, avec statut OAuth par provider)
  - Corriger l'affirmation "OS keyring" → "encrypted blob in library database (XChaCha20-Poly1305, per-library key)"
  - Supprimer la mention "cross-cloud moves" (implémenté seulement pour copies via streaming, pas de server-side cross-account)
  - Ajouter section "OAuth Sign-in — Bring Your Own App" avec le flow browser + pointer vers le tutoriel intégré dans l'UI
  - Indiquer que OneDrive Business est hors scope (tenant=common uniquement pour MVP)
- [ ] Mettre à jour `.tasks/core/CLOUD-003-cloud-volume.md` :
  - Acceptance criteria "Files can be copied to and from the cloud volume" → `[x]` (après PR 2 mergée)
  - "Next Steps" mis à jour : OAuth browser flow pour OneDrive est `Done`, Gdrive/Dropbox toujours `To Do`
  - Status → `Done` (puisque OneDrive MVP couvre l'epic)
- [ ] `.tasks/core/FILE-003-cloud-volume-file-operations.md` → `Done`
- [ ] Créer `.tasks/core/CLOUD-004-gdrive-dropbox-oauth.md` (successor task pour la généralisation Gdrive/Dropbox) avec status `To Do`
- [ ] Entrée CHANGELOG
- [ ] `.github/workflows/test.yml` : ajouter le nouveau test d'intégration aux runs CI (sans credentials live — mock uniquement)
- [ ] `cargo run --bin task-validator -- validate` passe

**Fichiers touchés :** `core/tests/`, `docs/core/cloud-integration.mdx`, `.tasks/core/*.md`, `.github/workflows/test.yml`, `CHANGELOG.md`.

---

## 5. Questions produit restantes

**Aucune bloquante** — l'architecture BYO supprime les 3 questions Q-MVP-1 à Q-MVP-3 du plan précédent. Seules deux questions mineures persistent, tranchables en cours de route :

### Q-MVP-A — Format du tutoriel : screenshots PNG vs captures en vidéo/GIF ?
Screenshots PNG = léger (quelques KB), image statique, accessibilité OK. Vidéo/GIF = plus engageant mais alourdit le bundle. **Reco : screenshots PNG en 2x pour Retina, ~200 KB total.**

### Q-MVP-B — Stockage des screenshots du tutoriel : `packages/assets/` ou inline base64 ?
`packages/assets/` est plus propre et permet le lazy loading. Base64 inline évite un round-trip. **Reco : `packages/assets/tutorials/onedrive/` avec lazy import dynamique.**

---

## 6. Risques du MVP

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| User bloqué à l'étape 3 du tutoriel (portail Azure confus) | Élevé | Abandon UX | Tutoriel intégré détaillé (PR 5), éventuellement video-capture post-MVP si feedback insuffisant |
| Rotated refresh tokens pas exposés par OpenDAL | Certain | Possible déconnexion user | PR 3 : OAuth refresh driven par Spacedrive, pas OpenDAL. `CloudBackend` reçoit l'access_token déjà rafraîchi via `CloudCredentialManager` |
| Delta token expire silencieusement après une semaine inactive | Moyen | Full resync inattendu | PR 6 : gestion 410 + full resync auto + métrique `cloud_sync_state.consecutive_failures` |
| Microsoft redirect URI exact-match casse sur port occupé | Moyen | User-facing error | PR 4 : essayer 5 ports (53682-53686), fail gracieux avec message clair si tous occupés |
| `jiff::Timestamp` conversion bug sur timezone edge case | Faible | mtime faux | PR 1 : test unitaire qui round-trip un DateTime avec timezone non-UTC |
| Windows CRLF sur `.rs` (documenté dans AGENTS global) | Certain | Diff git sales | Toujours vérifier `git diff` avant commit, `git checkout -- <file>` pour artifacts |

---

## 7. Critères de merge de chaque PR

Avant toute demande de merge :

- [ ] `cargo build` passe
- [ ] `cargo clippy -- -D warnings` passe
- [ ] `cargo test` passe (y compris les nouveaux tests)
- [ ] `cargo fmt --check` passe
- [ ] `bun typecheck` (ou équivalent) passe côté TS
- [ ] Types TS regénérés si types Rust publics modifiés (`cargo run --bin generate_typescript_types`)
- [ ] Pas de `println!`, pas de `.unwrap()` dans paths de production (seulement en tests)
- [ ] Pas de `panic!` sauf cas "invariant violé à la compilation qu'on peut prouver"
- [ ] Docs publiques sur chaque item `pub` (`///` avec au moins 1 phrase expliquant pourquoi)
- [ ] Task `.tasks/core/` correspondante mise à jour (checkboxes + status + last_updated)
- [ ] `cargo run --bin task-validator -- validate` passe

---

## 8. Ordre d'exécution suggéré

```
PR 1 (Foundation) ──┬─── PR 2 (Fix panics + create_folder)
                    │
                    └─── PR 3 (OAuth infra BYO) ─── PR 4 (OneDrive provider) ─── PR 5 (UI tutoriel)
                                                                                  │
                                                             PR 6 (Delta API) ────┤
                                                                                  │
                                                             PR 7 (fixes UI) ─────┤
                                                                                  │
                                                             PR 8 (tests + docs) ─┘
```

- **PR 1 est bloquante** pour PR 2 et PR 3 (layers, features, metadata)
- **PR 2 et PR 3 peuvent partir en parallèle** après PR 1
- **PR 5 dépend de PR 4** (types TS générés, actions disponibles)
- **PR 6 peut commencer en parallèle de PR 5** (backend only)
- **PR 7 peut être fait à tout moment après PR 5**
- **PR 8 en dernier** (tests E2E dépendent de tout)

**Estimation totale :** 3-4 semaines dev plein temps solo, 2 semaines en parallèle à 2 personnes.

---

## 9. Hors scope — stubs et TODOs explicites à laisser dans le code

Au fil des PRs, ces `TODO(cloud-mvp)` sont acceptables (et préférables à du code demi-fait). Chaque TODO :
- Préfixé `TODO(cloud-mvp):`
- Cite le scope
- Pointe vers cette doc : `see .investigations/cloud-drives/06-mvp-onedrive-vertical-slice.md#section`

Liste exhaustive des TODOs autorisés :
1. Google Drive OAuth browser flow (PR 3 infra prête, provider à écrire)
2. Dropbox OAuth browser flow (idem)
3. Spacedrive-owned public client_ids (changement de Phase 2+, à discuter avec founder quand opportun)
4. Settings page "Connected Accounts" (Phase 2)
5. Reconnect / Edit credentials UI (Phase 2 — actuellement : delete + re-add)
6. OneDrive Business / Work accounts (tenant-specific)
7. `CloudGroup` renderer sidebar (désactivé temporairement)
8. LIST-diff change detection pour S3-family
9. Dropbox longpoll overlay
10. Device-code flow pour CLI-over-SSH
11. Server-side cross-backend copy (S3 bucket A → S3 bucket B)
12. Initial sync progress UX fine (actuel : existing naive progress)
13. Pré-remplissage Azure portal via URL params (approche C, tutoriel Phase 2)

---

## 10. Au-delà du MVP

Une fois ce MVP mergé et stabilisé :
1. **Dupliquer pattern pour Gdrive** (nouveau provider dans `core/src/ops/cloud/oauth/providers/gdrive.rs` + tutoriel Gdrive). Attention `drive.file` scope pour éviter OAuth verification Google. ~1 semaine.
2. **Dupliquer pattern pour Dropbox** (fixed redirect URI → enregistrer 5 ports spécifiques). ~1 semaine.
3. **Settings "Connected Accounts"** + reconnect UX.
4. **LIST-diff** pour S3/GCS/Azure.
5. **Discussion avec founder** : créer des Spacedrive-owned public client_ids pour les 3 providers ? Si oui, constantes hardcodées remplacent les inputs, tutoriel devient optionnel via setting "Advanced: use my own OAuth app".
6. Évaluer Dropbox longpoll en fonction de la télémétrie sur les autres providers.

---

**Prochaine action :** go/no-go sur le découpage des PRs → je commence PR 1.
