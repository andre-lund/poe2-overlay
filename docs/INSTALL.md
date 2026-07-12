# poe2-overlay — install & launch (CachyOS + KDE Plasma 6 Wayland)

The build-and-package runbook for the overlay itself (T6). For the *PathofTrading*
reference-tool setup see `docs/research/INSTALL.md`; for the architecture see the ADRs
in `docs/adr/` and the glossary in `CONTEXT.md`.

Target stack: KDE Plasma 6 on native Wayland (KWin), PoE2 via Steam Proton. The overlay
is a `wlr-layer-shell` surface KWin composites over the fullscreen game (ADR-0001).

## 1. Build

```fish
npm install            # first time only
npm run tauri:build
```

Produces three bundles under `src-tauri/target/release/bundle/` — `appimage/`, `deb/`,
`rpm/` — plus the bare binary at `src-tauri/target/release/poe2-overlay`. The
`tauri:build` script sets `GDK_BACKEND=wayland` + `WEBKIT_DISABLE_DMABUF_RENDERER=1`
(required for the layer-shell surface on KDE Wayland), plus two flags that keep the
AppImage step from failing: `APPIMAGE_EXTRACT_AND_RUN=1` (bundle without FUSE, e.g. in a
container/CI) and `NO_STRIP=1` (skip linuxdeploy's strip pass — it errors on the bundled
WebKit/GStreamer libs on this stack; yields a larger but reliably-built AppImage). The
local install uses the bare release binary regardless, so an AppImage failure never
blocks it.

Build deps (all already present on this machine per plan T1): `webkit2gtk-4.1`, `gtk3`,
`gtk-layer-shell`, `base-devel`, `openssl`, `librsvg`, `libappindicator-gtk3`.

## 2. Install

```fish
./packaging/install.sh
```

User-level and idempotent (re-run after a rebuild to refresh). It installs the release
binary to `~/.local/bin/poe2-overlay` (preferred for a local install — lighter and faster
to spawn than the AppImage, which is the portable artifact for sharing; the AppImage is
used only if the bare binary isn't built), writes two action launchers + an autostart
entry, and registers the KWin shortcuts:

| What | Where | Purpose |
|---|---|---|
| binary | `~/.local/bin/poe2-overlay` | the app (warm instance + flag forwarders) |
| `poe2-overlay-pricecheck.desktop` | `~/.local/share/applications/` | `Exec=… --price-check` |
| `poe2-overlay-hide.desktop` | `~/.local/share/applications/` | `Exec=… --hide` (panic dismiss) |
| `poe2-overlay-runes.desktop` | `~/.local/share/applications/` | `Exec=… --runes` (rune price sheet) |
| `poe2-overlay.desktop` | `~/.config/autostart/` | starts the **warm** instance on login |
| `_launch` shortcuts | `kglobalshortcutsrc` | `Ctrl+Alt+D` price-check · `Ctrl+Alt+X` hide · `Ctrl+Alt+F` rune sheet |

Override the keys: `POE2_PRICECHECK_KEY="Meta+D" POE2_HIDE_KEY="Meta+X" POE2_RUNES_KEY="Meta+F"
./packaging/install.sh` (KDE shortcut syntax), or change them later in System Settings →
Keyboard → Shortcuts.

The installer also fetches **Fontin** (exljbris, PoE's UI typeface) into
`~/.local/share/fonts/fontin/` — free for personal and commercial *use* but not
redistributable, so it is downloaded from the author's site rather than shipped in this
repo. Purely cosmetic: the overlay theme falls back to system serifs when it's absent.

> The regex cheat-sheet (T8) is **disabled for now**; its old `Ctrl+Alt+F` key now opens the
> rune price sheet (T9) — poe.ninja rune prices for the active league, for reward panels the
> game offers no clipboard copy on. The regex backend + Vue panel are retained, dormant, for
> an easy restore (ADR-0006).

**Why autostart matters (ADR-0002):** the KDE shortcuts launch `poe2-overlay --price-check`,
and `tauri-plugin-single-instance` forwards that flag to the *already-running* instance.
So one warm instance must exist first — autostart provides it. (Triggering a shortcut with
no instance running would start a fresh one that drops its own flag and just sits idle
holding `/dev/uinput`; autostart avoids that.)

## 3. /dev/uinput access (only if needed)

The in-game copy is a `uinput` virtual keyboard, which needs write access to
`/dev/uinput` (no `input` group — the session ACL is enough; ADR-0002). Check:

```fish
getfacl /dev/uinput   # want a line like  user:<you>:rw-
```

If it's not writable, `install.sh` prints the privileged fix (run it yourself):

```fish
sudo install -m644 packaging/99-poe2-overlay-uinput.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## 4. Activate

**Log out and back in.** This is the reliable step: `kglobalacceld` only loads
`kglobalshortcutsrc` at session start, so the new shortcuts don't fire until it reloads,
and the autostart entry brings the warm instance up. To try it the same session without
relogging, start the warm instance by hand:

```fish
env GDK_BACKEND=wayland WEBKIT_DISABLE_DMABUF_RENDERER=1 ~/.local/bin/poe2-overlay &
```

(but the shortcuts still need the relogin to register).

## 5. Use

1. Launch PoE2 (Steam/Proton).
2. Hover an item, press **Ctrl+Alt+D** — the overlay shows the cheapest listings (or, for
   a **waystone**, a Safe/Caution/Dangerous/Deadly mod verdict — ADR-0005).
3. In the price card: pick a **league**, toggle **stat / base filters** and edit min/max,
   then **Requery**.
4. **✕** or **Ctrl+Alt+X** dismisses the overlay.

Currency/stackables price instantly via poe.ninja (no GGG quota); gear/waystones hit the
GGG trade2 API, which self-throttles on its rate-limit headers (ADR-0004).

## Troubleshooting

- **Shortcut does nothing** → you didn't relogin after install (step 4); `kglobalacceld`
  hasn't loaded the new keys. Verify the binding exists:
  `kreadconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-pricecheck.desktop --key _launch`.
- **"no item under the cursor" / empty** → the XWayland clipboard sync is racy; the app
  already polls ~800 ms. Make sure you're hovering an item when you press the key.
- **Overlay doesn't appear over the game** → it's confirmed over borderless/windowed
  fullscreen. If *exclusive* fullscreen ever hides it, use borderless, or set
  `KWIN_DRM_NO_DIRECT_SCANOUT=1` in the game's launch env (ADR-0001 / plan notes).
- **Item copy disabled** → `/dev/uinput` not writable (step 3); the app logs
  `cannot open /dev/uinput`.

## Uninstall

```fish
rm -f ~/.local/bin/poe2-overlay \
      ~/.local/share/applications/poe2-overlay-{pricecheck,hide,regex}.desktop \
      ~/.config/autostart/poe2-overlay.desktop \
      ~/.local/share/icons/hicolor/128x128/apps/io.olund.poe2overlay.png
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-pricecheck.desktop --key _launch --delete
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-hide.desktop --key _launch --delete
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-regex.desktop --key _launch --delete
# then re-login. Remove the udev rule too if you added it:
#   sudo rm -f /etc/udev/rules.d/99-poe2-overlay-uinput.rules
```
