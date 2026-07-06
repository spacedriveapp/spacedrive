# aitkis fork — branch & merge strategy

Fork: [arik103/spacedrive](https://github.com/arik103/spacedrive)  
Upstream: [spacedriveapp/spacedrive](https://github.com/spacedriveapp/spacedrive)

## Remotes

| Remote | URL | Use |
|--------|-----|-----|
| `origin` | `https://github.com/arik103/spacedrive.git` | Push your work |
| `upstream` | `https://github.com/spacedriveapp/spacedrive` | Pull official updates |

## Branches

| Branch | Role |
|--------|------|
| `main` | Mirror of upstream. Sync only — **no custom commits**. |
| `feature/arik103` | **Long-lived dev branch.** All fork customizations live here. |
| `feature/arik103/<topic>` | Short-lived branches for a specific change; merge back into `feature/arik103`. |

## Rules

1. **Always develop on `feature/arik103`** (or a topic branch off it).
2. **Never commit custom work to `main`.**
3. **Topic branches** branch from `feature/arik103`, merge back into `feature/arik103` when done.
4. **Run the app from `feature/arik103`** — `scripts/aitkis-dev-browser.sh` enforces this.

## Sync upstream into your work

```bash
# 1. Update fork main from original
git fetch upstream
git checkout main
git merge upstream/main
git push origin main

# 2. Bring upstream into long-lived branch
git checkout feature/arik103
git merge main
# resolve conflicts, test, then:
git push origin feature/arik103
```

Prefer **merge** (not rebase) on `feature/arik103` when pulling from `main` — keeps history readable for a long-lived branch.

## Topic branch workflow

```bash
git checkout feature/arik103
git pull origin feature/arik103
git checkout -b feature/arik103/hidden-files

# ... work, commit ...
git push -u origin feature/arik103/hidden-files

# when done:
git checkout feature/arik103
git merge feature/arik103/hidden-files
git push origin feature/arik103
git branch -d feature/arik103/hidden-files   # optional cleanup
```

## Push / pull cheat sheet

```bash
git push origin feature/arik103          # push your dev branch
git pull origin feature/arik103          # pull your dev branch
git fetch upstream                       # check for upstream changes (no merge)
```

## First-time setup (already done if you cloned the fork)

```bash
git remote rename origin upstream        # only if origin still points at spacedriveapp
git remote add origin https://github.com/arik103/spacedrive.git
git fetch origin
git checkout -b feature/arik103
git push -u origin feature/arik103
```
