# Cloud Drives — Plan d'implémentation (synthèse recherche + état des lieux)

**Date:** 2026-04-18
**Branche:** `feature/cloud-drives-investigation`
**Input:** 4 rapports d'état des lieux (`01-04`) + 4 rapports de recherche (`research/01-04`)
**Statut:** Plan — aucun code modifié.

---

## 1. Où on en est, où on va

L'investigation a établi (rapport `00-executive-summary.md`) que Cloud-as-a-Volume est à ~55 % d'un MVP utilisable, avec 5 blocants critiques. La phase recherche (`research/`) donne les réponses techniques nécessaires pour les lever. Voici la carte blocant → solution :

| Blocant (rapport 00) | Réponse (rapport research) | Effort |
|---|---|---|
| #1 `FileCopyJob.panic!` sur cloud | `op.copy(src, dst)` ou `op.rename()` natif OpenDAL si `Capability::write_can_copy`/`rename`. Fallback stream `reader()` → `writer_with().concurrent().chunk()`. **Gate sur Capability avant d'appeler.** (research/02 §3, §4) | 2-3 jours |
| #2 Zéro OAuth | Loopback 127.0.0.1:ephemeral + PKCE S256 + `oauth2 = "5"` + `webbrowser` — implémenté dans `sd-daemon`, PAS via `tauri-plugin-oauth`. (research/01 §2, §4) | 5-7 jours |
| #3 Restart perd B2/Wasabi/Spaces | Retirer le `_ =>` catch-all (`manager.rs:335-338`), ajouter les constructors manquants en s'appuyant sur la matrice provider (research/02 §4, research/03 §2) | 1 jour |
| #4 `create_folder` déconnecté | Câbler `CreateFolderAction` sur `CloudBackend::create_directory` qui existe déjà (`cloud.rs:412-432`) | 2h |
| #5 Indexing ré-hash tout | Polling + ETag-diff pour S3-family ; Changes/Delta/Cursor pour Gdrive/OneDrive/Dropbox ; persistence dans 2 tables neuves (`cloud_sync_state` + `cloud_object_cache`) (research/03 §4, §5) | 10-15 jours |

## 2. Décisions architecturales (avec sources)

### 2.1 OAuth : loopback + PKCE dans sd-daemon

**Décision :** implémentation directe dans `sd-daemon`, pas de plugin Tauri.

**Justification :**
- RFC 8252 exige PKCE pour tout client public natif, impose loopback sur 127.0.0.1, interdit les WebViews embedded (Google les bloque activement avec `disallowed_useragent`) — research/01 §3
- `oauth2` crate v5 expose `PkceCodeChallenge::new_random_sha256()` + `set_pkce_verifier()` proprement — research/01 §4
- `tauri-plugin-oauth` est un wrapper fin stagnant (v2.0.0, 2024-11-05) qui exécute le serveur loopback *dans le webview*. Or Spacedrive découple daemon ↔ UI — le daemon doit rester le propriétaire de l'intégration cloud (compatible CLI/headless) — research/01 §5
- Tous les grands (rclone, Insync, Cyberduck, Nextcloud) suivent ce même pattern — research/04 §2.1

**Composants Rust à ajouter :**
- `oauth2 = "5"` (confirmé par research/01 §4)
- `webbrowser = "1"` (launch default browser)
- Réutiliser `reqwest` et `tokio::net::TcpListener` déjà présents

### 2.2 Client secrets : publics dans le binaire, avec override BYO

**Décision :** ship les client IDs Spacedrive dans le binaire, documentés comme publics. Fournir un flag `--client-id` / setting UI pour override.

**Justification :**
- Google lui-même écrit : "for installed apps, the client secret is obviously not treated as a secret" — research/01 §7
- Dropbox + Microsoft exigent/recommandent le flow "public client" (PKCE, pas de secret) — research/01 §6
- rclone, Insync, Cyberduck, Nextcloud suivent ce pattern — research/01 §6, research/04 §2.1
- L'override BYO sert (a) aux entreprises qui exigent leur propre OAuth app, (b) aux early users pour contourner la limite 7 jours de Google

### 2.3 OpenDAL : bump 0.54 → 0.55, ajout layers, exploiter Metadata

**Décisions :**
1. Bump à 0.55 — breaking changes limités à `jiff::Timestamp` sur 2 sites (`cloud.rs:334` et `:361`) — research/02 §2
2. Ajouter stack de layers par défaut : `RetryLayer` + `TimeoutLayer` + `ConcurrentLimitLayer` + `TracingLayer` — research/02 §7
3. Gate toutes les opérations sur `Capability` avant appel (rename ne marche pas sur S3/Azblob/GCS) — research/02 §3
4. Persister `etag`, `version`, `content_md5` dans le cache d'objets (clé de change detection) — research/02 §3, research/03 §5
5. Utiliser `writer_with(path).chunk(8 MiB).concurrent(4)` pour tout upload > 8 MiB — research/02 §4

### 2.4 Change detection : per-provider state machine

**Décision :** architecture en 3 stratégies, table unique `cloud_sync_state` avec discriminateur.

| Provider | Stratégie | Table d'état |
|---|---|---|
| Google Drive | `changes.list` + `startPageToken` | `cloud_sync_state.change_token` |
| OneDrive / SharePoint | `/drive/root/delta` + `deltaLink` | `cloud_sync_state.change_token` |
| Dropbox | `list_folder/continue` + longpoll 480s | `cloud_sync_state.change_token` |
| S3 / R2 / MinIO / B2 / Wasabi / Spaces | LIST + diff vs `cloud_object_cache` | `cloud_object_cache` |
| Azure Blob | LIST + diff (Change Feed si activé, out-of-scope MVP) | `cloud_object_cache` |
| GCS | LIST + `generation` diff | `cloud_object_cache` |

**Sources :** research/03 §2, §3, §4

**Points importants :**
- Gdrive et OneDrive push notifications exigent un endpoint HTTPS public → impossible depuis desktop → polling only — research/03 §2
- Dropbox longpoll sur `notify.dropboxapi.com` est le SEUL moyen desktop d'avoir du near-realtime — research/03 §2
- Checkpoint du cursor **après chaque page réussie**, pas seulement en fin de scan — sinon une indexation interrompue de 1M fichiers repart de zéro — research/03 §6

### 2.5 Identité fichier : ID opaque backend > path

**Décision :** stocker `(volume_id, provider_file_id, etag_or_version)` comme clé primaire d'un fichier cloud dans `entries`. Le path devient un index secondaire.

**Justification :**
- Gdrive (`fileId`), OneDrive (`id`), Dropbox (`FileMetadata.id`) ont un ID stable à travers rename/move/reparent — research/03 §9
- S3/Azure/GCS n'ont pas d'ID stable : rename = DELETE + PUT. Spacedrive doit soit accepter la perte d'annotations, soit faire du matching content-hash — research/03 §9
- C'est le pattern Nextcloud (`oc-fileid`) et object_store (`UpdateVersion`) — research/04 §2.3

**Implication :** `cloud_identifier` sur `volumes` stocke la racine (bucket / drive ID / root folder ID). Un nouveau champ `provider_file_id` sur `entries` stocke l'ID par fichier.

### 2.6 "Account" comme concept > "Volume"

**Décision :** introduire une abstraction `CloudAccount` au-dessus de `Volume`.

**Justification :**
- Un token OAuth Google expose My Drive + Shared Drives + Shared-With-Me — chacun peut être un volume distinct mais partage l'auth — research/04 §2.4
- C'est le mental model utilisateur (Cyberduck "Bookmark", rclone "remote")
- Ça simplifie le reconnect/revoke : un seul point au lieu d'un par volume

**Schéma proposé :**
```
CloudAccount (1) ──── (N) Volume
   │
   ├── provider: CloudServiceType
   ├── oauth_credentials_id (FK vers cloud_credentials)
   ├── display_name
   ├── created_at
   └── last_refresh_at
```

### 2.7 Capabilities : pattern rclone à adopter

**Décision :** introduire `VolumeBackendCapability` + `BackendFeatures` struct inspirée de rclone.

**Source :** research/04 §2.1, §2.2 — pattern validé sur 60+ backends pendant 10 ans.

**Structure proposée :**
```rust
pub struct BackendFeatures {
    pub server_side_copy: bool,       // op.copy() sans download+upload
    pub server_side_rename: bool,     // op.rename()
    pub stable_file_id: bool,         // supporte file_id opaque
    pub change_notifications: ChangeNotificationKind, // None | Polling | LongPoll | Webhook
    pub content_hash: Option<HashAlgorithm>, // MD5 (S3), SHA256 (B2), content_hash (Dropbox), etc.
    pub case_insensitive: bool,       // OneDrive = true, S3 = false
    pub supports_duplicates: bool,    // Gdrive peut avoir 2 fichiers même nom dans 1 dossier
    pub max_file_size: Option<u64>,
    pub multipart_threshold: Option<u64>,
    // ... plus selon besoin
}
```

## 3. Roadmap phasée

### Phase 0 — Foundation (1-2 semaines)

**But :** préparer le terrain avant tout feature-work.

- [ ] Bump OpenDAL 0.54 → 0.55 (2 sites `cloud.rs:334`, `:361` sur `jiff::Timestamp`)
- [ ] Ajouter stack de layers : `RetryLayer` + `TimeoutLayer` + `ConcurrentLimitLayer` + `TracingLayer` sur toute construction d'`Operator`
- [ ] Ajouter `BackendFeatures` struct + `VolumeBackend::features(&self) -> &BackendFeatures`
- [ ] Exposer `etag`, `version`, `content_md5` dans `BackendMetadata` (ajouter au trait)
- [ ] Fixer `CloudBackend::exists()` qui swallow les erreurs d'auth en `false` (bug research/02 §8)
- [ ] Introduire tables `cloud_accounts`, `cloud_sync_state`, `cloud_object_cache` (migrations)
- [ ] Retirer le `_ =>` catch-all de `manager.rs:335-338` → blocant #3

### Phase 1 — MVP "ça marche pour un utilisateur" (3-4 semaines)

**But :** un utilisateur non-dev peut connecter Google Drive, voir ses fichiers, les copier, sans crash.

**Auth (research/01) :**
- [ ] Crate `oauth2 = "5"` + `webbrowser` dans sd-daemon
- [ ] Module `core/src/ops/cloud/oauth/` : `start_flow`, `wait_for_callback`, `exchange_code`, `refresh_token`
- [ ] Action `cloud.oauth.start` → renvoie `{auth_url, flow_id}` ; UI ouvre l'URL via `webbrowser::open`
- [ ] Action `cloud.oauth.complete` pollée par l'UI → renvoie `{account_id}` quand le callback est reçu
- [ ] Provider configs (3 fichiers) : Google (`scope: drive.file`, endpoint issuer), Microsoft (`scope: Files.ReadWrite.All offline_access`, tenant `common`), Dropbox (`token_access_type=offline`)
- [ ] Ship public client_ids dans config hardcodée + override via env var / setting
- [ ] Frontend : remplacer `AddStorageModal.tsx:1349-1394` paste-tokens par un bouton "Connect via Browser" qui poll l'action
- [ ] Page Settings "Connected Accounts" (manquait totalement — rapport 03 §4)

**Fixes critiques :**
- [ ] `FileCopyJob` cloud branch : remplacer les 2 `panic!` (`job.rs:1042`, `:1485`) par copy via OpenDAL `op.copy()` si capable, sinon stream `reader()` → `writer_with().concurrent()` — blocant #1
- [ ] Câbler `CreateFolderAction` sur `CloudBackend::create_directory` — blocant #4
- [ ] Unifier les deux `getVolumeIcon` (`volumeIcons.ts:71` canonique, supprimer `DevicePanel.tsx:55-72`) — rapport 03 §3
- [ ] Câbler `volumes.remove_cloud` dans `useVolumeContextMenu.ts:60-68` — rapport 03 §3
- [ ] Retirer ou implémenter `GroupType::Cloud` (bug UI actif : bouton dans `AddGroupModal.tsx:49` sans renderer) — rapport 03 §3

**Token refresh :**
- [ ] Background task qui rafraîchit les tokens N minutes avant expiry (research/01 §7)
- [ ] Handle du 401 → tentative de refresh → sinon marquer account `needs_reauth` et notifier l'UI
- [ ] Persistence du refresh_token rotated (Google rotate parfois) — research/02 §8

**Tests :**
- [ ] Tests d'intégration minimum par provider via `services::Memory` pour la logique, et un set de tests live optionnels derrière `#[ignore]`
- [ ] Test restart rehydration pour les 6 providers

### Phase 2 — Sync mature (3-4 semaines)

**But :** ré-indexation rapide, change detection, annotations survivent aux renames.

**State machine (research/03) :**
- [ ] Trait `ChangeDetectionStrategy` avec 3 impls : `DeltaTokenStrategy` (Gdrive/OneDrive/Dropbox), `ListDiffStrategy` (S3-family/GCS/Azure), `LongpollOverlay` (Dropbox)
- [ ] Scheduler : 60s foreground, 5min background, backoff exponentiel sur 429/503
- [ ] Checkpoint du cursor après chaque page (pas chaque scan complet)
- [ ] Gestion invalidation token : HTTP 410 `resyncRequired` (OneDrive), `reset` error (Dropbox), 4xx (Gdrive) → full resync
- [ ] Initial sync pacer : rate-limit par provider, pas par CPU/disk

**Indexer intégration :**
- [ ] Réécrire `core/src/ops/indexing/phases/processing.rs:270-288` pour interroger le `cloud_object_cache` avant de marquer `Change::New`
- [ ] `core/src/ops/indexing/path_resolver.rs:227` : résoudre cloud paths via `provider_file_id` (actuellement `None`)

**File identity :**
- [ ] Colonne `provider_file_id` sur `entries` (NULL pour S3-family, required pour Gdrive/OneDrive/Dropbox)
- [ ] Lookup order : `provider_file_id` d'abord, path fallback
- [ ] Migration des entries cloud existantes (ou simple full reindex après merge)

**Dropbox longpoll :**
- [ ] Task dédiée par volume Dropbox qui maintient une connexion longpoll
- [ ] Fallback sur polling classique si le longpoll fail

### Phase 3 — Polish, éventail de providers, UX (3-4 semaines)

**Providers additionnels :**
- [ ] WebDAV (rclone-like, grand public Nextcloud/Seafile)
- [ ] SFTP (OpenDAL supporte)
- [ ] Évaluer : pCloud, Box, Mega (demande user-base validée)

**UX :**
- [ ] Progress reporting du initial sync (1M fichiers = plusieurs heures)
- [ ] "Reconnect" workflow quand refresh fail
- [ ] Indicateur provider-specific sur les fichiers (cloud source)
- [ ] Preview streaming pour vidéos cloud (sans download complet)

**Docs :**
- [ ] Réécrire `docs/core/cloud-integration.mdx` pour refléter la réalité (liste vraie des providers, vraie architecture crypto, vraie change detection) — résoudre les 5 contradictions listées rapport 00 §7

**Observability :**
- [ ] Métriques : requêtes par provider, rate-limit hits, erreurs OAuth, taille du cache d'objets
- [ ] Logs structurés par `account_id` et `volume_id`

## 4. Questions produit ouvertes (à trancher avec le founder)

### Q1 — Cloud-as-a-Peer : on-hold ou abandonné ?
Les tasks CLOUD-000/001/002 restent listées mais le code iroh a été supprimé. Le post-mortem V1 (`docs/overview/history.mdx:114`) considère le focus cloud comme une erreur business. **Décider** si on ferme formellement les 3 tasks ou si on planifie une reprise post-MVP Cloud-as-a-Volume.

### Q2 — Google Drive scope : `drive.file` (MVP rapide) vs `drive` (all-access)
`drive.file` limite aux fichiers créés ou ouverts par Spacedrive → pas besoin d'OAuth verification Google → tokens long-lived immédiatement. Mais l'UX est dégradée (un user attend de voir TOUT son Drive).
`drive` exige OAuth verification + CASA assessment → ~3-6 mois de process Google, $15k-75k pour CASA tier 2/3.
**Recommandation :** commencer `drive.file` pour MVP, planifier verification en parallèle. Source research/01 §6.

### Q3 — BYO client credentials dans l'UI ?
Ship-only ou ship-with-override. Recommandé : ship public + setting "Advanced → Use my own OAuth app" (1 champ par provider). Research/01 §8.

### Q4 — Rename tracking pour S3-family : heuristique ou loss-accepted ?
Pas d'ID stable côté provider. Options :
- Accepter que tags/notes se perdent sur rename (simple, honest)
- Heuristique content-hash (match par taille + hash dans une fenêtre temporelle) — research/03 §9
**Recommandation :** MVP = loss-accepted + warning UI, v2 = heuristique opt-in.

### Q5 — Change Feed Azure Blob et GCS Pub/Sub : supportés ?
Nécessitent bucket-owner rights. La plupart des users n'ont que des creds read-only. Polling LIST-diff reste obligatoire comme fallback. **Décision :** polling-only en MVP, event-subscriptions éventuellement en Phase 3 sur opt-in avancé.

### Q6 — Frontend OAuth : Tauri deep link backup ?
Recommandé non (loopback suffit, aucun provider ne recommande custom scheme). Mais enregistrer `spacedrive://` quand même dans `tauri.conf.json` coûte rien et servirait à autre chose (magic links, share links). À trancher.

### Q7 — CLI-over-SSH OAuth ?
User en SSH sur un serveur distant ne peut pas ouvrir un browser local. Pattern device-code-flow de Microsoft/Google existe. Post-MVP.

## 5. Risques identifiés

| Risque | Impact | Mitigation |
|---|---|---|
| Google refuse OAuth verification (app trop générique) | Bloque toute adoption sérieuse de Gdrive | Commencer `drive.file` scope qui évite verification. Préparer dossier verification en parallèle. |
| Dropbox rate-limits sur longpoll connections | Déconnexions, near-realtime perdu | Fallback polling déjà prévu. Jitter + backoff. |
| OneDrive delta token expire silencieusement | Ré-indexation complète inattendue | Détection 410 + resync auto + métrique pour alerter |
| OpenDAL 0.56 breaking changes avant qu'on ship | Retravail | Locker à 0.55 jusqu'à post-MVP |
| Rotated refresh tokens non exposés par OpenDAL | Déconnexion utilisateur après N jours | Driver OAuth out-of-band côté Spacedrive, ne pas laisser OpenDAL gérer le refresh — research/02 §8 |
| CASA assessment coûteux pour scope `drive` complet | Feature bloquée | `drive.file` scope en MVP, CASA planifié avec business case |

## 6. Stack technique final

| Domaine | Choix | Source |
|---|---|---|
| OAuth client | `oauth2 = "5"` | research/01 §4 |
| Browser launch | `webbrowser = "1"` | research/01 §4 |
| Loopback server | `tokio::net::TcpListener` + `hyper` (déjà dans l'arbre) | research/01 §4 |
| Cloud storage | `opendal = "0.55"` (bump depuis 0.54) | research/02 §2 |
| HTTP | `reqwest` (déjà présent) | — |
| Credential crypto | `XChaCha20-Poly1305` déjà en place (crypto/cloud_credentials.rs) | rapport 02 |
| DB | SQLite + SeaORM (déjà présent) | — |
| Pattern backend trait | rclone `Fs` + `Features` (porté en Rust) | research/04 §2.1 |
| Pattern change detection | rclone per-provider + object_store ETag | research/03 §3, research/04 §2.2 |

## 7. Références

### Rapports d'état des lieux (`.investigations/cloud-drives/`)
- `00-executive-summary.md` — synthèse état des lieux
- `01-design-and-vision.md` — tasks, docs, roadmap
- `02-backend-implementation.md` — Rust core audit
- `03-frontend-implementation.md` — Tauri/TS audit
- `04-maturity-and-history.md` — git history, tests, CI

### Rapports de recherche (`.investigations/cloud-drives/research/`)
- `01-oauth-native-apps.md` — RFC 8252, PKCE, Tauri, provider specs
- `02-opendal-deep-dive.md` — API complète, provider matrix, upgrade 0.54→0.55
- `03-change-detection-and-sync.md` — algorithmes, schéma DB, rate limits
- `04-reference-implementations.md` — rclone, object_store, Nextcloud, Syncthing, Cyberduck, Kopia

### Externes clés (tous datés avril 2026)
- RFC 8252 — https://datatracker.ietf.org/doc/html/rfc8252
- OpenDAL — https://docs.rs/opendal/0.55/opendal/
- `oauth2` crate — https://docs.rs/oauth2/latest/
- Google Drive OAuth — https://developers.google.com/identity/protocols/oauth2/native-app
- Microsoft Graph Auth — https://learn.microsoft.com/en-us/graph/auth-v2-user
- Dropbox OAuth — https://developers.dropbox.com/oauth-guide
- rclone source — https://github.com/rclone/rclone
- Apache Arrow object_store — https://docs.rs/object_store/

---

**Prochaine étape proposée :** revue de ce plan avec le founder pour trancher Q1-Q7, puis démarrage Phase 0 sur une série de petits PR (chacun ≤ 400 LOC) pour restaurer la CI verte avant les gros changements de Phase 1.
