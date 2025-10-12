# Mintlify Documentation Integration Plan

## Overview

Integrate Mintlify docs at https://docs.spacedrive.com while keeping docs in the main repo (not a separate docs repo).

## Architecture

```
spacedriveapp/spacedrive/              # Main repo (source of truth)
├── docs/
│   ├── core/*.md                       # Core architecture docs
│   ├── cli/*.md                        # CLI documentation
│   ├── troubleshooting/*.md           # Troubleshooting guides
│   ├── design/                        # EXCLUDED from published docs
│   ├── whitepaper.md
│   ├── philosophy.md
│   ├── history.md
│   └── sdk.md

spacedriveapp/docs/                    # Mintlify deployment repo (submodule)
├── mint.json                          # Mintlify config
├── docs -> ../docs/                   # Symlink to main repo docs
├── .gitignore                         # Ignore design/ folder
└── .github/workflows/mintlify.yml    # Auto-deploy on push
```

## Implementation Steps

### 1. Create Mintlify Repo

```bash
# Create new repo on GitHub: spacedriveapp/docs
cd ~/Projects
gh repo create spacedriveapp/docs --public --clone

cd docs
git init
```

### 2. Set Up Mintlify

```bash
cd ~/Projects/docs

# Initialize Mintlify
npm i -g mintlify
mintlify init

# This creates mint.json and example docs
```

### 3. Configure Symlink Strategy

**Option A: Symlink entire docs folder (Recommended)**

```bash
cd ~/Projects/docs

# Remove example docs
rm -rf docs/

# Create symlink to main repo's docs
ln -s ~/Projects/spacedrive/docs ./docs

# Create .gitignore to exclude design folder from publishing
cat > .gitignore << EOF
docs/design/
*.DS_Store
EOF

git add .
git commit -m "Initial Mintlify setup with symlinked docs"
git push origin main
```

**Option B: Selective symlinks (More control)**

```bash
cd ~/Projects/docs
mkdir -p docs

# Symlink only what you want published
ln -s ~/Projects/spacedrive/docs/core docs/core
ln -s ~/Projects/spacedrive/docs/cli docs/cli
ln -s ~/Projects/spacedrive/docs/troubleshooting docs/troubleshooting
ln -s ~/Projects/spacedrive/docs/whitepaper.md docs/whitepaper.md
ln -s ~/Projects/spacedrive/docs/philosophy.md docs/philosophy.md
ln -s ~/Projects/spacedrive/docs/history.md docs/history.md
ln -s ~/Projects/spacedrive/docs/sdk.md docs/sdk.md

# Don't symlink design/ - it stays private
```

### 4. Convert .md to .mdx

Mintlify prefers MDX for components. Two approaches:

**Quick (Keep as .md):**
Mintlify supports plain Markdown. Just use `.md` files - they work fine.

**Future-proof (Convert to .mdx):**
```bash
cd ~/Projects/spacedrive/docs

# Rename all .md to .mdx (safe, MDX is superset of MD)
find . -name "*.md" -type f -not -path "./design/*" -exec bash -c 'mv "$0" "${0%.md}.mdx"' {} \;

# Update any references
git grep -l '\.md' | xargs sed -i '' 's/\.md/.mdx/g'

git commit -am "Convert docs to MDX for Mintlify"
```

### 5. Configure mint.json

```json
{
  "name": "Spacedrive",
  "logo": {
    "dark": "/logo/dark.svg",
    "light": "/logo/light.svg"
  },
  "favicon": "/favicon.png",
  "colors": {
    "primary": "#2AB673",
    "light": "#55D799",
    "dark": "#0D9373",
    "anchors": {
      "from": "#2AB673",
      "to": "#55D799"
    }
  },
  "topbarLinks": [
    {
      "name": "GitHub",
      "url": "https://github.com/spacedriveapp/spacedrive"
    }
  ],
  "topbarCtaButton": {
    "name": "Download",
    "url": "https://spacedrive.com/download"
  },
  "tabs": [
    {
      "name": "Core",
      "url": "core"
    },
    {
      "name": "CLI",
      "url": "cli"
    },
    {
      "name": "SDK",
      "url": "sdk"
    }
  ],
  "anchors": [
    {
      "name": "Discord",
      "icon": "discord",
      "url": "https://discord.gg/gTaF2Z44f5"
    },
    {
      "name": "GitHub",
      "icon": "github",
      "url": "https://github.com/spacedriveapp/spacedrive"
    }
  ],
  "navigation": [
    {
      "group": "Getting Started",
      "pages": [
        "docs/whitepaper",
        "docs/philosophy",
        "docs/history"
      ]
    },
    {
      "group": "Core Architecture",
      "pages": [
        "docs/core/architecture",
        "docs/core/library",
        "docs/core/domain-models",
        "docs/core/indexing",
        "docs/core/locations",
        "docs/core/devices",
        "docs/core/networking",
        "docs/core/pairing",
        "docs/core/sync",
        "docs/core/library-sync",
        "docs/core/tagging",
        "docs/core/virtual-sidecars",
        "docs/core/ops",
        "docs/core/events",
        "docs/core/database",
        "docs/core/testing",
        "docs/core/volume-system"
      ]
    },
    {
      "group": "CLI",
      "pages": [
        "docs/cli/cli",
        "docs/cli/index-verify"
      ]
    },
    {
      "group": "SDK",
      "pages": [
        "docs/sdk"
      ]
    },
    {
      "group": "Troubleshooting",
      "pages": [
        "docs/troubleshooting/file-descriptors"
      ]
    }
  ],
  "footerSocials": {
    "github": "https://github.com/spacedriveapp/spacedrive",
    "discord": "https://discord.gg/gTaF2Z44f5",
    "twitter": "https://twitter.com/spacedriveapp"
  },
  "analytics": {
    "plausible": {
      "domain": "docs.spacedrive.com"
    }
  }
}
```

### 6. Add to Main Repo as Submodule

```bash
cd ~/Projects/spacedrive

# Add docs repo as submodule
git submodule add https://github.com/spacedriveapp/docs.git mintlify-docs

# This creates mintlify-docs/ folder pointing to spacedriveapp/docs repo
```

**Why submodule?**
- Keeps deployment config (mint.json, Mintlify settings) separate
- Main repo doesn't get polluted with Mintlify infrastructure
- Docs repo can deploy independently via Mintlify's GitHub integration

### 7. Set Up Auto-Deploy

Mintlify auto-deploys from GitHub. Configure in Mintlify dashboard:

1. Go to https://dashboard.mintlify.com
2. Connect `spacedriveapp/docs` repo
3. Set branch: `main`
4. Set root directory: `/`
5. Custom domain: `docs.spacedrive.com`

Mintlify watches the repo - any push to `main` auto-deploys.

### 8. Workflow for Updating Docs

**Scenario: Edit core/architecture.md**

```bash
cd ~/Projects/spacedrive

# Edit docs as usual in main repo
vim docs/core/architecture.md

# Commit to main repo
git add docs/core/architecture.md
git commit -m "Update architecture docs"
git push

# Docs repo auto-updates via symlink
cd mintlify-docs
git add docs/core/architecture.md  # Follows symlink
git commit -m "Update architecture docs (via main repo)"
git push

# Mintlify auto-deploys
```

**Optional: Automate with Git Hook**

```bash
# In main repo: .git/hooks/post-commit
#!/bin/bash
# Auto-sync docs changes to Mintlify repo

if git diff-tree --name-only --no-commit-id -r HEAD | grep '^docs/'; then
  echo "Docs changed, syncing to Mintlify repo..."
  cd mintlify-docs
  git add docs/
  git commit -m "Sync: $(git log -1 --pretty=%B ../)"
  git push
  cd ..
fi
```

## Alternative: Unified Repo Approach

If you don't want a separate docs repo, keep Mintlify config IN the main repo:

```
spacedrive/
├── docs/              # Documentation (source of truth)
├── mint.json          # Mintlify config at root
└── .github/
    └── workflows/
        └── mintlify-deploy.yml
```

**Pros:**
- Single repo, simpler
- No submodule complexity

**Cons:**
- Mintlify watches main repo (more CI noise)
- mint.json pollutes root directory
- Harder to exclude design/ folder from Mintlify

## Recommended: Submodule Approach

**Why:**
1. **Separation of concerns**: Main repo = code, docs repo = published docs
2. **Clean exclusions**: design/ folder never touches Mintlify repo
3. **Independent deployment**: Docs can redeploy without triggering main repo CI
4. **Mintlify best practice**: They recommend separate docs repos

## Migration Checklist

- [ ] Create `spacedriveapp/docs` repo
- [ ] Set up Mintlify project at dashboard.mintlify.com
- [ ] Create mint.json with navigation structure
- [ ] Symlink `spacedrive/docs/` into Mintlify repo
- [ ] Add .gitignore to exclude design/
- [ ] Convert .md → .mdx (optional, can do later)
- [ ] Add docs repo as submodule to main repo
- [ ] Configure custom domain: docs.spacedrive.com
- [ ] Test deployment
- [ ] Set up Plausible analytics (optional)
- [ ] Add "Documentation" link to main README

## Domain Setup

```bash
# DNS settings for docs.spacedrive.com
CNAME docs -> mintlify.app

# Or if using Cloudflare:
CNAME docs -> <your-mintlify-subdomain>.mintlify.app
```

Mintlify provides the target - check dashboard after connecting repo.

## Post-Launch

Once published:

1. **Add to README:**
   ```markdown
   - **📖 Read the [Documentation](https://docs.spacedrive.com)**
   ```

2. **Deprecate internal docs/:**
   - Keep docs/ as source of truth
   - Point contributors to docs.spacedrive.com for browsing
   - docs/ becomes "raw source", docs.spacedrive.com is "published view"

3. **Version docs for V2:**
   Mintlify supports versioning:
   ```json
   "versions": ["v2", "v1"]
   ```

## Example File Structure After Setup

```
~/Projects/
├── spacedrive/                        # Main repo
│   ├── docs/                          # Source of truth
│   │   ├── core/*.mdx
│   │   ├── cli/*.mdx
│   │   ├── design/                    # NOT published
│   │   ├── whitepaper.mdx
│   │   └── ...
│   ├── mintlify-docs/                 # Git submodule → spacedriveapp/docs
│   │   ├── docs -> ../docs/           # Symlink
│   │   ├── mint.json
│   │   └── .gitignore (design/)
│   └── README.md

└── docs/                              # Mintlify repo (if you clone separately)
    ├── docs -> ~/Projects/spacedrive/docs/
    └── mint.json
```

## Quick Start Commands

```bash
# 1. Create and setup docs repo
gh repo create spacedriveapp/docs --public --clone
cd ~/Projects/docs
npm i -g mintlify
mintlify init
rm -rf docs/
ln -s ~/Projects/spacedrive/docs ./docs
echo "docs/design/" > .gitignore

# 2. Add to main repo as submodule
cd ~/Projects/spacedrive
git submodule add https://github.com/spacedriveapp/docs.git mintlify-docs

# 3. Configure Mintlify dashboard
# - Connect spacedriveapp/docs
# - Set custom domain: docs.spacedrive.com
# - Deploy!
```

Let me know if you want me to generate the complete `mint.json` with all your current docs auto-detected!
