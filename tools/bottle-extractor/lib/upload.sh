#!/bin/bash
# upload.sh - Upload extracted binaries to S3-compatible storage
#
# Uploads the extracted package as a .tar.gz archive to S3
# Generates URLs for use in Burd service definitions

# Load S3 credentials from config file
load_s3_config() {
    local config_file="$SCRIPT_DIR/.s3config"

    if [[ ! -f "$config_file" ]]; then
        log_error "S3 config not found: $config_file"
        log_error "Create it with: ./upload.sh --init"
        return 1
    fi

    source "$config_file"

    if [[ -z "$S3_ACCESS_KEY" ]] || [[ -z "$S3_SECRET_KEY" ]]; then
        log_error "S3 credentials not configured in $config_file"
        return 1
    fi

    return 0
}

# Initialize S3 config file
init_s3_config() {
    local config_file="$SCRIPT_DIR/.s3config"

    if [[ -f "$config_file" ]]; then
        log_warn "Config already exists: $config_file"
        read -p "Overwrite? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return 1
        fi
    fi

    cat > "$config_file" << 'EOF'
# S3 Configuration for Bottle Extractor
# This file contains secrets - DO NOT COMMIT TO GIT

S3_ACCESS_KEY=""
S3_SECRET_KEY=""
S3_REGION="fr-par"
S3_ENDPOINT="https://burdbin.s3.fr-par.scw.cloud"
S3_BUCKET="burdbin"
EOF

    chmod 600 "$config_file"
    log_success "Created $config_file"
    log_info "Edit the file and add your credentials"
    return 0
}

# Generate S3 signature for request
# Usage: s3_signature <method> <path> <date> <content_type>
s3_signature() {
    local method="$1"
    local path="$2"
    local date="$3"
    local content_type="${4:-application/octet-stream}"

    local string_to_sign="${method}\n\n${content_type}\n${date}\n/${S3_BUCKET}${path}"
    echo -en "$string_to_sign" | openssl sha1 -hmac "$S3_SECRET_KEY" -binary | base64
}

# Upload a file to S3
# Usage: upload_to_s3 <local_file> <s3_path>
upload_to_s3() {
    local local_file="$1"
    local s3_path="$2"
    local content_type="${3:-application/gzip}"
    local acl="public-read"

    local date
    date=$(date -u "+%a, %d %b %Y %H:%M:%S GMT")

    # Include x-amz-acl in signature
    local string_to_sign="PUT\n\n${content_type}\n${date}\nx-amz-acl:${acl}\n/${S3_BUCKET}${s3_path}"
    local signature
    signature=$(echo -en "$string_to_sign" | openssl sha1 -hmac "$S3_SECRET_KEY" -binary | base64)

    local url="${S3_ENDPOINT}${s3_path}"

    curl -X PUT \
        -H "Host: ${S3_BUCKET}.s3.${S3_REGION}.scw.cloud" \
        -H "Date: ${date}" \
        -H "Content-Type: ${content_type}" \
        -H "x-amz-acl: ${acl}" \
        -H "Authorization: AWS ${S3_ACCESS_KEY}:${signature}" \
        --data-binary "@${local_file}" \
        "$url" \
        --fail --silent --show-error

    return $?
}

# Create archive and upload extracted package
# Usage: upload_package <output_dir>
upload_package() {
    local output_dir="$1"

    if [[ ! -d "$output_dir" ]]; then
        log_error "Output directory not found: $output_dir"
        return 1
    fi

    # Read manifest to get package info
    local manifest="$output_dir/manifest.json"
    if [[ ! -f "$manifest" ]]; then
        log_error "Manifest not found: $manifest"
        return 1
    fi

    local name version arch
    name=$(jq -r '.name' "$manifest")
    version=$(jq -r '.version' "$manifest")
    arch=$(jq -r '.architecture' "$manifest")

    # Create archive filename
    local archive_name="${name}-${version}-${arch}.tar.gz"
    local archive_path="/tmp/${archive_name}"

    log_info "Creating archive: $archive_name"

    # Create tarball from the output directory
    # Archive contents will be at root level (bin/, lib/, etc/)
    tar -czf "$archive_path" -C "$output_dir" .

    local size
    size=$(ls -lh "$archive_path" | awk '{print $5}')
    log_info "Archive size: $size"

    # Upload to S3
    local s3_path="/${name}/${version}/${archive_name}"

    log_info "Uploading to: ${S3_ENDPOINT}${s3_path}"

    if upload_to_s3 "$archive_path" "$s3_path"; then
        log_success "Upload complete!"

        local public_url="${S3_ENDPOINT}${s3_path}"
        echo ""
        echo "Download URL:"
        echo "  $public_url"
        echo ""
        echo "For Burd service registry:"
        echo "  {"
        echo "    \"version\": \"$version\","
        echo "    \"url\": \"$public_url\","
        echo "    \"arch\": \"$arch\","
        echo "    \"sha256\": \"$(shasum -a 256 "$archive_path" | awk '{print $1}')\""
        echo "  }"

        # Clean up
        rm -f "$archive_path"
        return 0
    else
        log_error "Upload failed!"
        rm -f "$archive_path"
        return 1
    fi
}

# List uploaded packages
list_packages() {
    local date
    date=$(date -u "+%a, %d %b %Y %H:%M:%S GMT")

    local signature
    signature=$(s3_signature "GET" "/" "$date" "")

    curl -X GET \
        -H "Host: ${S3_BUCKET}.s3.${S3_REGION}.scw.cloud" \
        -H "Date: ${date}" \
        -H "Authorization: AWS ${S3_ACCESS_KEY}:${signature}" \
        "${S3_ENDPOINT}/" \
        --silent | xmllint --format - 2>/dev/null || cat
}
