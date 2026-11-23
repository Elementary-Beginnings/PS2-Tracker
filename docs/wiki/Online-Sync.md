# Online Sync

For members of the ELME outfit: the tracker can upload your finished sessions
to the ELME stats server, **stats.elme.games**, so you can see them there.
Online sync is **off by default**. Everything stays on your computer as before;
the upload is extra.

## Switching it on

1. *Settings → Online sync* → tick *Upload my finished sessions to
   stats.elme.games* and confirm.
2. *Sign in with Discord*. The tracker opens Discord in your browser; allow the
   sign-in and go back to the tracker. The browser shows a short "Signed in"
   page that you can close.
3. The settings show your Discord ID, the ELME characters linked to your
   account and the upload queue.

Your Discord account has to be linked to an ELME character first (verification
in the ELME Discord). Without that the server answers "not linked" and the
tracker says so.

## What is uploaded, and when

- **When:** after a session has ended – on logout once the resume window ran
  out, after inactivity, or when you end it yourself. A paused session (logged
  out, waiting for a relogin) is not finished yet. A session that ends while the
  app is closed (crash) is queued at the next start.
- **What:** the complete session as the JSON export writes it
  ([session file format](https://github.com/Elementary-Beginnings/ps2-tracker/blob/main/docs/session-file.schema.json)):
  stats, weapons, vehicles, the kill feed **with the names and character IDs of
  the other players**, server and continent, achievements, the pause times.
- **Whose:** only sessions of your **own** characters (*Characters* page) that
  the server lists as linked to your Discord account. Sessions of squad or
  outfit members you track are **never** uploaded.
- **Older sessions** are not uploaded by themselves. *Upload existing
  sessions…* queues every older session of your linked characters that was
  never uploaded.
- A session you open again with *Resume last session* is uploaded again when it
  ends; the new version replaces the old one on the server.

## Status in the history

With online sync on, *History* has an *Online* column:

| Shows | Meaning |
|---|---|
| – | not synced (never queued) |
| queued | waiting for the upload or for the next attempt (the tooltip shows the last error) |
| synced | the server has this version (the tooltip shows when it was uploaded) |
| failed | the server rejected it for a reason a repeat does not fix (the tooltip shows the reason) |

In the detail view of a session, *Upload now* queues it (again) and *Retry
upload* repeats a failed one.

Uploads run in the background, one after the other, with their own connection;
they never delay the Census requests or the user interface. Network errors and
server errors are retried with a growing pause (30 s up to 1 h); when the server
asks to slow down (HTTP 429), the tracker waits as long as the server says
(`Retry-After`). Rejections (character not linked, file too large, invalid
session) are not repeated automatically.

## Sign-in and keyring

- Discord OAuth 2 with PKCE, scope `identify` (your Discord ID only). The
  tracker never sees a Discord password or token and holds no client secret.
- The browser comes back to a local address on your own computer
  (`http://127.0.0.1:47821/callback`, or port 47822/47823 if that one is taken).
  The tracker listens there only during the sign-in (at most 5 minutes), only on
  127.0.0.1, and closes the port right after.
- The stats server returns an access token (valid 15 minutes, kept only in
  memory) and a refresh token (valid 30 days, single use).
- The refresh token is stored **only in your system keyring**: on Linux any
  keyring with the freedesktop Secret Service interface (for example GNOME
  Keyring, or KWallet with its Secret Service interface), on Windows the
  Credential Manager. It is never written to `config.toml`, the log or an error
  message.
- **No keyring, no sign-in:** if no keyring is available (or it refuses
  access), the tracker does not sign in and shows why. It never falls back to a
  file. Install or unlock a keyring and try again.
- Flatpak: the app may talk to `org.freedesktop.secrets` on the session bus for
  this (see [Permissions and Privacy](Permissions-and-Privacy#flatpak)).

## Signing out and deleting data

- *Sign out* revokes the sign-in on the server and deletes the keyring entry.
  Uploaded sessions stay on the server.
- *Delete my online data…* deletes your account and every uploaded session on
  stats.elme.games and signs you out. Your local sessions stay.
- Switching online sync off stops all uploads at once; the queue waits.
- When you leave ELME (no linked character any more), the server stops
  accepting uploads and deletes your data after 30 days unless you are verified
  again.

The privacy policy of the stats server is linked in the settings
(*Privacy policy*).

## Advanced

*Settings → Online sync → Advanced*:

- **API address** – default `https://stats.elme.games/api/v1`. Only `https://`
  addresses are accepted (plain `http://localhost` only in development builds).
  Each address has its own sign-in in the keyring.
- **Discord client ID** – empty uses the ID built into official builds. Builds
  without one (self-built) need the client ID of the server's Discord
  application to sign in.

Configuration (`config.toml`):

```toml
[online_sync]
enabled = false
api_base = "https://stats.elme.games/api/v1"
discord_client_id = ""
```

## What the server checks

The server accepts a session only for a character linked to your account, only
finished sessions, at most 25 MiB, and validates the file with the tracker's
own code. It allows 60 uploads per hour per account. The API is described in
the server repository (`documentation/online-sync-api.md`).
