#!/bin/bash
# Check Caddy certificate chain

CA_ROOT="$HOME/Library/Application Support/Caddy/pki/authorities/local/root.crt"
CA_INTERMEDIATE="$HOME/Library/Application Support/Caddy/pki/authorities/local/intermediate.crt"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

echo "=== Caddy PKI Certificates ==="
echo ""

echo "1. ROOT CA:"
openssl x509 -in "$CA_ROOT" -noout -subject -issuer -dates 2>/dev/null || echo "   Not found"
echo ""

echo "2. INTERMEDIATE CA:"
openssl x509 -in "$CA_INTERMEDIATE" -noout -subject -issuer -dates 2>/dev/null || echo "   Not found"
echo ""

echo "=== What's currently in the login keychain ==="
security find-certificate -a -c "Caddy" "$KEYCHAIN" 2>/dev/null | grep -E "labl|alis" || echo "No Caddy certs found"
echo ""

echo "=== Testing: Add INTERMEDIATE cert to keychain ==="
echo "Run this to trust the intermediate:"
echo "  security add-trusted-cert -k \"$KEYCHAIN\" -r trustAsRoot \"$CA_INTERMEDIATE\""
echo ""

echo "=== Or trust BOTH root and intermediate ==="
echo "  security add-trusted-cert -k \"$KEYCHAIN\" -r trustRoot \"$CA_ROOT\""
echo "  security add-trusted-cert -k \"$KEYCHAIN\" -r trustAsRoot \"$CA_INTERMEDIATE\""
