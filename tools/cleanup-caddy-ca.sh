#!/bin/bash
# Script to remove all Caddy Local Authority certificates from System keychain

KEYCHAIN="/Library/Keychains/System.keychain"
CERT_NAME="Caddy Local Authority"

echo "Looking for '$CERT_NAME' certificates in System keychain..."

# Count how many exist
COUNT=$(security find-certificate -a -c "$CERT_NAME" "$KEYCHAIN" 2>/dev/null | grep -c "keychain:")

if [ "$COUNT" -eq 0 ]; then
    echo "No certificates found."
    exit 0
fi

echo "Found $COUNT certificate(s). Removing them one by one..."

# Keep deleting until none remain
while security find-certificate -c "$CERT_NAME" "$KEYCHAIN" >/dev/null 2>&1; do
    echo "Deleting a '$CERT_NAME' certificate..."
    security delete-certificate -c "$CERT_NAME" "$KEYCHAIN" 2>/dev/null
    if [ $? -ne 0 ]; then
        echo "Failed to delete. You may need to unlock the keychain first."
        exit 1
    fi
done

echo "All '$CERT_NAME' certificates have been removed."
