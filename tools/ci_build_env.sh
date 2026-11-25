# Sourced by the release workflow before a build (bash). Maps the optional
# repository values to the compile-time variables of the app and defines
# check_embedded. Values are never printed (GitHub also masks secrets).
#   CENSUS_DEFAULT_SERVICE_ID (secret) -> PS2_DEFAULT_SERVICE_ID
#   SYNC_DISCORD_CLIENT_ID (variable, public; the ELME Discord application:
#     online sync sign-in and default Rich Presence application)
#     -> PS2_SYNC_DISCORD_CLIENT_ID
unset PS2_DEFAULT_SERVICE_ID PS2_DEFAULT_DISCORD_APP_ID PS2_SYNC_DISCORD_CLIENT_ID
if [ -n "${CENSUS_DEFAULT_SERVICE_ID:-}" ]; then
  case "$CENSUS_DEFAULT_SERVICE_ID" in
    *[!A-Za-z0-9_]*) echo "CENSUS_DEFAULT_SERVICE_ID has invalid characters - ignored, build uses example" ;;
    *) export PS2_DEFAULT_SERVICE_ID="$CENSUS_DEFAULT_SERVICE_ID"; echo "built-in Census service ID: set" ;;
  esac
else
  echo "built-in Census service ID: not set (example)"
fi
if [ -n "${SYNC_DISCORD_CLIENT_ID:-}" ]; then
  if [[ "$SYNC_DISCORD_CLIENT_ID" =~ ^[0-9]{17,20}$ ]]; then
    export PS2_SYNC_DISCORD_CLIENT_ID="$SYNC_DISCORD_CLIENT_ID"
    echo "ELME Discord application (online sync, Rich Presence default): set"
  else
    echo "SYNC_DISCORD_CLIENT_ID is not 17-20 digits - ignored"
  fi
else
  echo "ELME Discord application: not set (online sync and Rich Presence only with an own ID)"
fi

# check_embedded <binary>: the built-in service ID is in the binary (proof
# without printing it); nothing to check without one.
check_embedded() {
  if [ -n "${PS2_DEFAULT_SERVICE_ID:-}" ]; then
    if grep -qF "$PS2_DEFAULT_SERVICE_ID" "$1"; then
      echo "built-in default embedded: yes"
    else
      echo "built-in default embedded: NO"
      return 1
    fi
  fi
}
