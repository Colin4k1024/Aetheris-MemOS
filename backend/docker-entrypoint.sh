#!/bin/sh
set -e
mkdir -p /app/data
if ! touch /app/data/.write_check 2>/dev/null; then
    echo "Error: /app/data is not writable. Cannot start backend."
    exit 1
fi
rm -f /app/data/.write_check 2>/dev/null
exec /usr/local/bin/backend "$@"
