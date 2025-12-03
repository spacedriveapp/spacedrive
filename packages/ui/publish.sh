#!/bin/bash
set -e

# Temporarily rename for publishing
sed -i.bak 's/"name": "@sd\/ui"/"name": "@spacedriveapp\/ui"/' package.json
sed -i.bak 's/"@sd\/assets": "workspace:\*"/"@spacedriveapp\/assets": "^1.0.2"/' package.json

# Build and publish
bun run build
npm publish --access public

# Revert changes
mv package.json.bak package.json

echo "Published successfully!"
