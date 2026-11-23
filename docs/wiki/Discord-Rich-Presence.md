# Discord Rich Presence

![Updates and Discord](https://raw.githubusercontent.com/Elementary-Beginnings/ps2-tracker/main/docs/screenshots/integrations.png)

Shows your running session in your Discord status: character and faction, K/D,
kills, session time and optionally continent and server. Off by default.

## Setup

1. *Settings → Updates & Discord* → tick **Show my session in Discord**.
2. **Discord application ID:** leave the field empty to use the Discord
   application built into official builds. Your own application takes
   precedence: Discord Developer Portal → *New Application* → copy the
   *Application ID* (17–20 digits). Builds without a built-in application need
   an ID, otherwise the feature stays off.
3. Choose what to show (character and faction, kills, session time, continent
   and server).

## How it works

- Connection **only locally** to the running Discord app (Unix socket
  `$XDG_RUNTIME_DIR/discord-ipc-N`, on Windows the named pipe
  `\\.\pipe\discord-ipc-N`); the app opens no internet connection of its own
  for this.
- Updates at most every 15 seconds. If Discord is not running, the app retries
  quietly with a growing interval (up to 5 minutes).
- Continent and server appear only once the running session knows them.
- The built-in application ID (`PS2_DEFAULT_DISCORD_APP_ID` at build time) is a
  public client ID, not a secret.
- Flatpak: only `xdg-run/discord-ipc-0` and `xdg-run/discord-ipc-1` are
  allowed.

Details: [Permissions and Privacy](Permissions-and-Privacy#discord-optional-local).
