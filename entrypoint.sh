#!/bin/bash
set -e

PUID=${PUID:-1000}
PGID=${PGID:-1000}

groupadd -g "${PGID}" appuser 2>/dev/null || true
useradd -u "${PUID}" -g "${PGID}" -d /home -s /bin/bash appuser 2>/dev/null || true

chown -R appuser:appuser /home

if [ -d "/home/index" ]; then
    chown -R appuser:appuser /home/index
else
    mkdir -p /home/index
    chown -R appuser:appuser /home/index
    chmod 755 /home/index
fi

if [ -d "/home/source" ]; then
    chown -R appuser:appuser /home/source
else
    mkdir -p /home/source
    chown -R appuser:appuser /home/source
    chmod 755 /home/source
fi

exec gosu appuser fs-text-search-mcp --watch-dir /home/source --index-dir /home/index --quiet "$@"