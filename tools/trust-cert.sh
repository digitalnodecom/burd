#!/bin/bash
# Caddy CA Certificate Trust Manager
# Run: ./trust-cert.sh trust   - to add CA certs to keychain
# Run: ./trust-cert.sh untrust - to remove CA certs from keychain
# Run: ./trust-cert.sh status  - to check current status

CA_ROOT="$HOME/Library/Application Support/Caddy/pki/authorities/local/root.crt"
CA_INTERMEDIATE="$HOME/Library/Application Support/Caddy/pki/authorities/local/intermediate.crt"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

case "$1" in
  trust)
    echo "=== Adding Caddy CA certificates to login keychain ==="
    echo ""

    echo "1. Adding ROOT CA..."
    security add-trusted-cert -k "$KEYCHAIN" -r trustRoot "$CA_ROOT"
    echo "   Exit code: $?"

    echo ""
    echo "2. Adding INTERMEDIATE CA..."
    security add-trusted-cert -k "$KEYCHAIN" -r trustAsRoot "$CA_INTERMEDIATE"
    echo "   Exit code: $?"

    echo ""
    echo "=== Verifying ==="
    security find-certificate -a -c "Caddy" "$KEYCHAIN" | grep -E "labl|alis"
    echo ""
    echo "Done! You may need to restart your browser."
    ;;

  untrust)
    echo "=== Removing Caddy CA certificates from login keychain ==="

    # Remove root cert
    security delete-certificate -c "Caddy Local Authority - 2025 ECC Root" "$KEYCHAIN" 2>/dev/null
    security delete-certificate -c "Caddy Local Authority" "$KEYCHAIN" 2>/dev/null

    # Remove intermediate cert
    security delete-certificate -c "Caddy Local Authority - ECC Intermediate" "$KEYCHAIN" 2>/dev/null

    echo "Done!"
    ;;

  status)
    echo "=== Caddy CA Status ==="
    echo ""
    echo "1. Certificates in keychain:"
    security find-certificate -a -c "Caddy" "$KEYCHAIN" 2>/dev/null | grep -E "labl|alis" || echo "   No Caddy certificates found"
    echo ""
    echo "2. Certificate files exist:"
    ls -la "$CA_ROOT" 2>/dev/null || echo "   Root CA not found"
    ls -la "$CA_INTERMEDIATE" 2>/dev/null || echo "   Intermediate CA not found"
    echo ""
    echo "3. Trust verification (root):"
    security verify-cert -c "$CA_ROOT" 2>&1
    echo ""
    echo "4. Trust verification (intermediate):"
    security verify-cert -c "$CA_INTERMEDIATE" 2>&1
    ;;

  *)
    echo "Usage: $0 {trust|untrust|status}"
    echo ""
    echo "Commands:"
    echo "  trust   - Add both Root and Intermediate CA to login keychain"
    echo "  untrust - Remove all Caddy CA certs from login keychain"
    echo "  status  - Show current trust status"
    exit 1
    ;;
esac
