# Sourced by the release workflow before a build (bash). Maps the optional
# repository values to the compile-time variables of the app and defines
# check_embedded. Values are never printed (GitHub also masks secrets).
#   CENSUS_DEFAULT_SERVICE_ID (secret) -> PS2_DEFAULT_SERVICE_ID
#   DISCORD_APP_ID (variable, public)  -> PS2_DEFAULT_DISCORD_APP_ID
unset PS2_DEFAULT_SERVICE_ID PS2_DEFAULT_DISCORD_APP_ID
if [ -n "${CENSUS_DEFAULT_SERVICE_ID:-}" ]; then
  case "$CENSUS_DEFAULT_SERVICE_ID" in
    *[!A-Za-z0-9_]*) echo "CENSUS_DEFAULT_SERVICE_ID has invalid characters - ignored, build uses example" ;;
    *) export PS2_DEFAULT_SERVICE_ID="$CENSUS_DEFAULT_SERVICE_ID"; echo "built-in Census service ID: set" ;;
  esac
else
  echo "built-in Census service ID: not set (example)"
fi
if [ -n "${DISCORD_APP_ID:-}" ]; then
  if [[ "$DISCORD_APP_ID" =~ ^[0-9]{17,20}$ ]]; then
    export PS2_DEFAULT_DISCORD_APP_ID="$DISCORD_APP_ID"
    echo "built-in Discord application: set"
  else
    echo "DISCORD_APP_ID is not 17-20 digits - ignored"
  fi
else
  echo "built-in Discord application: not set"
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
