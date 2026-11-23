#!/usr/bin/env bash
# Passes the optional build-time defaults into the Flatpak build
# (workflow .github/workflows/flatpak.yml, job "build"):
#   tools/flatpak_cargo_env.sh write   before flatpak-builder
#   tools/flatpak_cargo_env.sh remove  afterwards (always)
# The flatpak-builder sandbox does not see the job's environment. Cargo's [env]
# in a temporary project-level .cargo/config.toml (not in the repository, not
# an artefact) hands the values to rustc. Deliberately not via build-options
# env in the manifest: flatpak-builder stores the manifest including env as
# /app/manifest.json in the bundle. Values are never printed.
set -euo pipefail

case "${1:-}" in
  write)
    # shellcheck source=tools/ci_build_env.sh
    . "$(dirname "$0")/ci_build_env.sh"
    mkdir -p .cargo
    : > .cargo/config.toml
    if [ -n "${PS2_DEFAULT_SERVICE_ID:-}" ] || [ -n "${PS2_DEFAULT_DISCORD_APP_ID:-}" ] \
      || [ -n "${PS2_SYNC_DISCORD_CLIENT_ID:-}" ]; then
      printf '[env]\n' >> .cargo/config.toml
      if [ -n "${PS2_DEFAULT_SERVICE_ID:-}" ]; then
        printf 'PS2_DEFAULT_SERVICE_ID = "%s"\n' "$PS2_DEFAULT_SERVICE_ID" >> .cargo/config.toml
      fi
      if [ -n "${PS2_DEFAULT_DISCORD_APP_ID:-}" ]; then
        printf 'PS2_DEFAULT_DISCORD_APP_ID = "%s"\n' "$PS2_DEFAULT_DISCORD_APP_ID" >> .cargo/config.toml
      fi
      if [ -n "${PS2_SYNC_DISCORD_CLIENT_ID:-}" ]; then
        printf 'PS2_SYNC_DISCORD_CLIENT_ID = "%s"\n' "$PS2_SYNC_DISCORD_CLIENT_ID" >> .cargo/config.toml
      fi
    fi
    ;;
  remove)
    rm -f .cargo/config.toml
    ;;
  *)
    echo "usage: $0 write|remove" >&2
    exit 2
    ;;
esac
