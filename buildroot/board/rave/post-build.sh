#!/bin/sh
set -eu

# Buildroot does not uninstall target files when a package is disabled during
# an incremental build. Keep the networkless rave image free of a stale
# ifupdown service from an older configuration.
rm -f "${TARGET_DIR}/etc/init.d/S40network"
