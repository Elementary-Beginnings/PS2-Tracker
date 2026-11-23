# Auto-Update

The **AppImage** and the **Windows installer** can update themselves.
Everything else updates through its package manager:

| Installation | Update |
|---|---|
| AppImage, Windows installer | in the app (below) |
| Arch package / AUR | `pacman -U` with the new package, or your AUR helper |
| Flatpak | install the new bundle (`flatpak install --user ps2-tracker.flatpak`) |
| Built from source | build again |

## In the app

*Settings → Updates & Discord*:

- **Check for updates on start** – off by default. Only when this is on, or
  when you press **Check now**, does the app ask for
  `https://github.com/Elementary-Beginnings/ps2-tracker/releases/latest/download/latest.json`.
  Nothing is sent besides the request itself.
- If a newer version exists, the app shows it with the changelog excerpt.
  **Install and restart** downloads the file, checks the signature and
  restarts; nothing is installed without this confirmation.

## Signatures

Update files are signed with the project's minisign key (Tauri updater, key ID
`9D64F76BBF11AD27`); the public key is built into the app, the private key
exists only as a GitHub Actions secret. A file is installed only with a valid
signature, and the signed version must match the announced one
(`requireSignedVersion`). Releases also carry `SHA256SUMS` and build provenance
attestations, see [Installation](Installation) and [Security](Security).

For package-manager and Flatpak installations the in-app updater is off; the
app then says how to update instead.
