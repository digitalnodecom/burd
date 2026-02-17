#!/bin/bash
# Bump version across all project files
# Usage: ./scripts/bump-version.sh 1.2.3

set -e

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "Usage: $0 <version>"
  echo ""
  echo "Examples:"
  echo "  $0 1.0.0"
  echo "  $0 1.1.0-beta.1"
  exit 1
fi

# Validate version format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Error: Invalid version format. Use semver (e.g., 1.2.3 or 1.2.3-beta.1)"
  exit 1
fi

echo "Bumping version to $VERSION"
echo ""

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Update Cargo.toml (main package)
echo "Updating src-tauri/Cargo.toml..."
sed -i '' 's/^version = ".*"/version = "'"$VERSION"'"/' src-tauri/Cargo.toml

# Update tauri.conf.json
echo "Updating src-tauri/tauri.conf.json..."
# Use node for reliable JSON editing
node -e "
const fs = require('fs');
const path = 'src-tauri/tauri.conf.json';
const config = JSON.parse(fs.readFileSync(path, 'utf8'));
config.version = '$VERSION';
fs.writeFileSync(path, JSON.stringify(config, null, 2) + '\n');
"

# Update package.json
echo "Updating package.json..."
node -e "
const fs = require('fs');
const path = 'package.json';
const pkg = JSON.parse(fs.readFileSync(path, 'utf8'));
pkg.version = '$VERSION';
fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
"

# Show what changed
echo ""
echo "Version updated to $VERSION in:"
echo "  - src-tauri/Cargo.toml"
echo "  - src-tauri/tauri.conf.json"
echo "  - package.json"
echo ""
echo "Next steps:"
echo "  1. Update CHANGELOG.md (move [Unreleased] to [$VERSION])"
echo "  2. git add -A && git commit -m \"chore: release v$VERSION\""
echo "  3. git tag v$VERSION"
echo "  4. git push && git push --tags"
