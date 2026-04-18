# Cloud Drives — État des lieux (synthèse)

**Date:** 2026-04-18
**Branche:** `feature/cloud-drives-investigation`
**Méthode:** Investigation en lecture seule. 4 agents de recherche spécialisés, rapports détaillés dans ce dossier.
**Verdict global:** **Scaffolding fonctionnel avec des bords cassés** — ~55 % vers un MVP utilisable. L'infrastructure est réelle et en main, mais l'UX est absente et le chemin d'écriture crashe le daemon.

---

## 1. Les 3 concepts "cloud" à ne pas confondre

Les docs et tasks utilisent le mot "cloud" pour trois choses très différentes :

| Concept | Tasks | Statut | Ce que ça veut dire |
|---|---|---|---|
| **Cloud-as-a-Volume** | CLOUD-003, VOL-004, FILE-003 | Partial MVP | Monter S3/Google Drive/OneDrive/Dropbox comme un volume natif via OpenDAL. Les fichiers restent chez le provider, Spacedrive les indexe et opère dessus. **C'est la seule piste qui a du code en production.** |
| **Cloud-as-a-Peer** | CLOUD-000, CLOUD-001, CLOUD-002 | Design only | Un `sd-core` isolé tourne dans l'infra Spacedrive et participe au réseau de sync P2P comme n'importe quel device. Ancienne implémentation iroh/`sd-cloud-schema` abandonnée (submodule retiré le 2025-11-14). Epic toujours listée mais pas relancée. |
| **Credential vault** | SEC-005 | Partial | Stockage chiffré des credentials (XChaCha20-Poly1305, library-scoped, SQLite). Fonctionnel mais la doc prétend utiliser l'OS keyring, ce qui est faux. |

Le reste du document parle **uniquement** de Cloud-as-a-Volume.

## 2. Fournisseurs supportés — Matrice réelle

| Provider | Backend câblé | Add via CLI/UI | Restart daemon | Auth utilisable | Bytes read | Bytes write | Copy/Move |
|---|---|---|---|---|---|---|---|
| S3 / Cloudflare R2 / MinIO | Oui | Oui | Oui | Clés IAM (OK) | Oui | Oui | **panic!** |
| Backblaze B2 | Oui | Oui | **Non** (manager.rs:335-338 warn!+continue) | Clés | Oui | Oui | **panic!** |
| Wasabi | Oui | Oui | **Non** | Clés | Oui | Oui | **panic!** |
| DigitalOcean Spaces | Oui | Oui | **Non** | Clés | Oui | Oui | **panic!** |
| Google Drive | Oui | Oui | Oui | **Paste tokens OAuth manuel** | Oui | Oui | **panic!** |
| OneDrive | Oui | Oui | Oui | **Paste tokens OAuth manuel** | Oui | Oui | **panic!** |
| Dropbox | Oui | Oui | Oui | **Paste tokens OAuth manuel** | Oui | Oui | **panic!** |
| Azure Blob | Oui | Oui | Oui | Clé compte | Oui | Oui | **panic!** |
| GCS | Oui | Oui | Oui | Service account JSON | Oui | Oui | **panic!** |
| pCloud / iCloud / MEGA / SharePoint / Box / Nextcloud / WebDAV | **Non** (listés en doc seulement) | — | — | — | — | — | — |

Doc `docs/core/cloud-integration.mdx:82-90` annonce "40+ providers" et tous ceux-ci. **9 backends seulement sont réellement câblés**, 6 sont bien testés en chemin complet.

## 3. Les 5 blocants critiques

### Blocant #1 — `FileCopyJob` panic! sur tout chemin cloud
- `core/src/ops/files/copy/job.rs:1042` et `:1485`
- `panic!("Cloud storage operations are not yet implemented")` — pas une `Err`, un vrai panic qui crashe le daemon
- Toute tentative utilisateur de copier/déplacer un fichier de ou vers un cloud → crash
- La doc `docs/core/cloud-integration.mdx:189` promet explicitement "cross-cloud moves" comme feature supportée. C'est faux.
- Task `FILE-003-cloud-volume-file-operations.md` encore en `To Do` (mais `FILE-000` prétend que c'est Done — contradiction).

### Blocant #2 — Zéro flow OAuth
- `grep -nEi "authorize|redirect|callback|localhost.*oauth"` sur `core/src/` et `apps/cli/` → **0 résultat**
- CLI : `apps/cli/src/domains/cloud/setup.rs:161-164` demande de coller `access_token` et `refresh_token` obtenus ailleurs
- UI : `AddStorageModal.tsx:1349-1394` affiche 4 inputs texte pour `client_id`, `client_secret`, `access_token`, `refresh_token`
- `tauri.conf.json` n'a pas d'entrée `deepLinks` → pas de callback possible depuis le browser
- **Google Drive / OneDrive / Dropbox sont developer-only** dans l'état actuel. Aucun utilisateur final ne peut s'en servir.
- Aucune task ne suit ce travail. C'est le trou le plus urgent et le moins documenté.

### Blocant #3 — Restart incomplet pour B2/Wasabi/Spaces
- `core/src/volume/manager.rs:335-338` : un `_ =>` catch-all qui `warn!` + `continue` sur tout `CloudServiceType` sans constructor de restauration
- Un utilisateur peut ajouter un volume B2 aujourd'hui ; après redémarrage du daemon, il disparaît silencieusement

### Blocant #4 — `create_folder` déconnecté
- `CloudBackend::create_directory` (`core/src/volume/backend/cloud.rs:412-432`) fonctionne
- Mais `CreateFolderAction` renvoie `ActionError::Internal("Cloud folder creation not yet implemented")` à `core/src/ops/files/create_folder/action.rs:120`
- Fix trivial, juste à câbler.

### Blocant #5 — Indexing ré-hash tout à chaque passe
- `core/src/ops/indexing/phases/processing.rs:270-288` traite chaque fichier cloud comme "new" à chaque indexation
- Pas d'intégration ETag / LastModified
- `core/src/ops/indexing/path_resolver.rs:227` renvoie `None` pour les cloud paths → tags/notes ne se résolvent pas silencieusement

## 4. UX frontend — Ce qui manque

- **Pas de page Settings "Connected Accounts"** — impossible d'auditer, reconnecter, révoquer sans un clic-droit sur un item de sidebar
- **`volumes.remove_cloud`** existe côté types (`generated/types.ts:4765`) mais **aucun code TS ne l'appelle** — les cloud volumes tombent dans le `untrack` générique de `useVolumeContextMenu.ts:60-68`
- **Bug UI actif** : `GroupType = "Cloud"` proposé dans `AddGroupModal.tsx:49` et `SpaceCustomizationPanel.tsx:241-242` mais **aucun renderer** dans `SpaceGroup.tsx` → sélectionner l'option crée un groupe vide invisible
- **Deux `getVolumeIcon` divergents** :
  - `packages/ts-client/src/volumeIcons.ts:71` — correct, parse le scheme (`s3://`, `gdrive://`, etc.)
  - `routes/overview/DevicePanel.tsx:55` — fragile, substring-match sur le display name ("S3", "Google", "Dropbox") → mauvais icône dès qu'on renomme un volume
- **Pas d'onboarding** cloud, pas d'empty state CTA, pas d'entrée sidebar dédiée

## 5. Tests & CI

- **5 tests au total** sur tout le périmètre cloud :
  - 2 integration tests OpenDAL en mémoire (`core/tests/delete_strategy_test.rs:221-299`)
  - 2 credential crypto round-trip (`cloud_credentials.rs` unit tests)
  - 1 test S3 live marqué `#[ignore]` (`core/src/volume/backend/cloud.rs:452`)
- **Zéro test** sur : OAuth, `VolumeAddCloudAction::execute`, rehydration au restart, indexation cloud end-to-end, token refresh
- **Zéro job CI** ne touche au cloud (`.github/workflows/*.yml`)

## 6. Chronologie & activité

- **Ligne actuelle** = réécriture from-scratch par Jamie Pine (22 commits sur 25 HEAD), 2025-10-13 → 2026-01-09
- Dernier commit substantif : `90476fb81` (2026-01-09, commentaires OAuth seulement)
- Depuis : uniquement des passes CI/typecheck/rename → **~3 mois de gel effectif**
- Task `CLOUD-003` toujours "In Progress" avec `last_updated: 2025-10-14`
- Ancienne implémentation iroh/`sd-cloud-schema` supprimée via `f7d7468bc` le 2025-11-14 — pas un revert explicite

## 7. Contradictions documentation ↔ code

| Doc promet | Réalité code |
|---|---|
| "40+ cloud providers supportés" (`volumes.mdx:75`, `cloud-integration.mdx:82-90`) | 9 enumérés, 6 testés |
| "Cross-cloud moves supportés" (`cloud-integration.mdx:189`) | `panic!` hardcodé |
| "Credentials stockés dans OS keyring" (`cloud-integration.mdx:117`) | Blob chiffré dans SQLite library DB (`cloud_credentials.rs:78-113`) |
| "Thumbnail / metadata caching pour cloud" (`cloud-integration.mdx`) | `is_cloud_path` skip dans le thumbnail job — rien de caché |
| "Change detection" / webhooks (`cloud-integration.mdx`) | Absent. Re-hash complet à chaque indexation |
| "Cost tracking" (`cloud-integration.mdx`) | Absent |

## 8. Estimation de complétude par couche

```
Credential encryption & storage     ████████████████████  95%
CloudBackend (OpenDAL adapter)      ████████████████████  95%
volumes.add_cloud action            ██████████████████░░  90%
DB schema & migrations              ████████████████████  95%
CLI setup flow                      ██████████████░░░░░░  70%
UI AddStorageModal                  ██████████████░░░░░░  70%
Provider matrix (6/9 câblés)        ████████████░░░░░░░░  60%
File read/list                      ████████████████████  95%
Indexing (cloud fast-path)          ████████░░░░░░░░░░░░  40%
File write/delete                   ████████████████░░░░  80%
File copy/move/rename               ░░░░░░░░░░░░░░░░░░░░   0% ← panic!
create_folder (cloud branch)        ██░░░░░░░░░░░░░░░░░░  10% (backend OK, action déconnectée)
Daemon restart rehydration (tous)   ████████████░░░░░░░░  60% (3 providers KO)
OAuth flow (end-to-end)             ░░░░░░░░░░░░░░░░░░░░   0%
Token refresh                       ░░░░░░░░░░░░░░░░░░░░   0%
Settings "Connected Accounts" UI    ░░░░░░░░░░░░░░░░░░░░   0%
Reconnect / edit credentials UI     ░░░░░░░░░░░░░░░░░░░░   0%
Tests (integration + e2e)           ██░░░░░░░░░░░░░░░░░░  10%
CI coverage                         ░░░░░░░░░░░░░░░░░░░░   0%
Change detection / webhooks         ░░░░░░░░░░░░░░░░░░░░   0%
```

## 9. Priorités suggérées pour la reprise

Deux piles bien distinctes à trancher :

### Piste A — "Terminer Cloud-as-a-Volume pour de vrai" (recommandé en premier)
Un utilisateur non-dev doit pouvoir connecter un Google Drive et travailler dessus sans crash.
1. Retirer les deux `panic!` dans `FileCopyJob` → `Err` propre + implémentation via OpenDAL `copy`/`rename`/streaming
2. Implémenter OAuth loopback local (redirect `http://127.0.0.1:PORT/callback`) + launch browser dans le CLI et dans Tauri
3. Câbler `volumes.remove_cloud` dans `useVolumeContextMenu`
4. Page Settings "Connected Accounts" avec reconnect/revoke
5. Corriger le catch-all de `manager.rs:335-338` pour B2/Wasabi/Spaces
6. Brancher `CreateFolderAction` sur `CloudBackend::create_directory`
7. Retirer ou implémenter `GroupType::Cloud`
8. Unifier les deux `getVolumeIcon`
9. Tests integration minimum sur les 6 providers
10. Resynchroniser docs ↔ code (rewrite `cloud-integration.mdx` pour refléter la réalité)

### Piste B — "Décider du sort de Cloud-as-a-Peer"
Question stratégique ouverte : le post-mortem V1 (`docs/overview/history.mdx:114`) dit que le focus cloud est une erreur de business model, et V2 pivote vers "premium extensions", mais l'epic CLOUD-000 est toujours listée. À trancher avec le founder avant de dépenser un octet dessus.

## 10. Sources

| Rapport | Focus | Fichier |
|---|---|---|
| Design & Vision | Tasks, docs, whitepaper, roadmap | [`01-design-and-vision.md`](./01-design-and-vision.md) |
| Backend Implementation | Rust core, ops, migrations, backends | [`02-backend-implementation.md`](./02-backend-implementation.md) |
| Frontend Implementation | Tauri, TS types, React UI, OAuth UX | [`03-frontend-implementation.md`](./03-frontend-implementation.md) |
| Maturity & History | Git history, tests, CI, placeholders | [`04-maturity-and-history.md`](./04-maturity-and-history.md) |

---

**Prochaine étape suggérée :** relire le rapport Design (01) pour aligner sur la vision, puis le Backend (02) pour prioriser les corrections critiques. Les rapports 03 et 04 donnent le contexte UX et l'historique.
