# Flathub shared modules

Copied unchanged from [flathub/shared-modules](https://github.com/flathub/shared-modules)
at commit `34add58f536223b780a3c9f077980cdaa51fc81f`:

- `libayatana-appindicator/` – libayatana-appindicator 0.6.1 with libdbusmenu,
  ayatana-ido and libayatana-indicator (GTK 3). The GNOME runtime does not ship
  it; the tray icon loads it at run time (`libayatana-appindicator3.so.1`).
- `intltool/` – build tool for libdbusmenu, removed from the bundle (`cleanup`).

The libraries are LGPL and built from their upstream sources (URLs and
checksums in the JSON files). To update: copy the directories again from a
newer commit of flathub/shared-modules and note the commit here.
