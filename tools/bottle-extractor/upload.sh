#!/bin/bash
# Upload extracted packages to S3
#
# Usage:
#   ./upload.sh <formula> [version] [arch]     Upload a package
#   ./upload.sh --init                          Initialize S3 config
#   ./upload.sh --list                          List uploaded packages
#
# Examples:
#   ./upload.sh redis                           Upload latest redis
#   ./upload.sh redis 8.4.0 arm64              Upload specific version
#   ./upload.sh --init                          Set up credentials

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$SCRIPT_DIR/lib"
OUTPUT_DIR="$SCRIPT_DIR/output"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Source library functions
source "$LIB_DIR/upload.sh"

# Handle --init flag
if [[ "$1" == "--init" ]]; then
    init_s3_config
    exit $?
fi

# Handle --list flag
if [[ "$1" == "--list" ]]; then
    if ! load_s3_config; then
        exit 1
    fi
    log_info "Listing packages in S3..."
    list_packages
    exit $?
fi

# Regular upload mode
FORMULA="${1:?Usage: $0 <formula> [version] [arch] OR $0 --init}"
VERSION="${2:-}"
ARCH="${3:-arm64}"

# Find the output directory
if [[ -n "$VERSION" ]]; then
    PACKAGE_DIR="$OUTPUT_DIR/$FORMULA/$VERSION-$ARCH"
else
    # Find latest version
    PACKAGE_DIR=$(find "$OUTPUT_DIR/$FORMULA" -maxdepth 1 -type d -name "*-$ARCH" | sort -V | tail -1)
fi

if [[ ! -d "$PACKAGE_DIR" ]]; then
    log_error "Package not found: $PACKAGE_DIR"
    log_info "Available packages:"
    find "$OUTPUT_DIR" -name "manifest.json" -exec dirname {} \; 2>/dev/null | while read dir; do
        echo "  - $dir"
    done
    exit 1
fi

log_info "Package directory: $PACKAGE_DIR"

# Load S3 config
if ! load_s3_config; then
    exit 1
fi

# Upload the package
upload_package "$PACKAGE_DIR"
