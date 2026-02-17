#!/bin/bash
# cleanup-ssl.sh - Run this to remove Caddy certs from keychain
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

echo "Removing Caddy certificates from keychain..."
security delete-certificate -c "Caddy Local Authority - 2025 ECC Root" "$KEYCHAIN" 2>/dev/null
security delete-certificate -c "Caddy Local Authority - ECC Intermediate" "$KEYCHAIN" 2>/dev/null
security delete-certificate -c "Caddy Local Authority" "$KEYCHAIN" 2>/dev/null

echo "Done! Verifying..."
security find-certificate -a -c "Caddy" "$KEYCHAIN" 2>/dev/null | grep -E "labl|alis" || echo "No Caddy certs remaining"
