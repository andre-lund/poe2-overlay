#!/usr/bin/env bash
# poe2-overlay installer — KDE Plasma 6 Wayland (T6).
#
# User-level and idempotent: re-run after a rebuild to refresh the binary + config.
# It writes only to your home (~/.local, ~/.config); the one privileged step (a
# /dev/uinput udev rule, only if your session ACL doesn't already grant access) is
# PRINTED for you to run, never executed here.
#
# It installs:
#   - the built binary           -> ~/.local/bin/poe2-overlay
#   - three action launchers     -> ~/.local/share/applications/poe2-overlay-{pricecheck,hide,runes}.desktop
#   - an autostart entry         -> ~/.config/autostart/poe2-overlay.desktop  (warm instance)
#   - three KWin global shortcuts -> kglobalshortcutsrc  (Ctrl+Alt+D / Ctrl+Alt+X / Ctrl+Alt+F)
#
# Keys are overridable: POE2_PRICECHECK_KEY / POE2_HIDE_KEY / POE2_RUNES_KEY (KDE syntax).
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"
ICON_DIR="$HOME/.local/share/icons/hicolor/128x128/apps"
BIN="$BIN_DIR/poe2-overlay"
APPID="io.olund.poe2overlay"
PRICE_KEY="${POE2_PRICECHECK_KEY:-Ctrl+Alt+D}"
HIDE_KEY="${POE2_HIDE_KEY:-Ctrl+Alt+X}"
RUNES_KEY="${POE2_RUNES_KEY:-Ctrl+Alt+F}"
# main.rs sets these in-process too; the launcher exports them as belt-and-suspenders
# so the GTK/WebKit backend is correct however the process is started (see ADR-0001).
WAYLAND_ENV="env GDK_BACKEND=wayland WEBKIT_DISABLE_DMABUF_RENDERER=1"

mkdir -p "$BIN_DIR" "$APP_DIR" "$AUTOSTART_DIR" "$ICON_DIR"

# 1. Install the built artifact. Prefer the bare release binary for a local install —
#    it's ~5x smaller and starts instantly (the AppImage adds mount/AppRun latency to
#    every flag-forwarder spawn) and uses the system WebKit we built against. The
#    AppImage is the portable/distribution artifact; fall back to it if that's all there is.
RELEASE_BIN="$REPO/src-tauri/target/release/poe2-overlay"
APPIMAGE="$(ls -t "$REPO"/src-tauri/target/release/bundle/appimage/*.AppImage 2>/dev/null | head -1 || true)"
if [[ -x "$RELEASE_BIN" ]]; then
    install -m755 "$RELEASE_BIN" "$BIN"
    echo "✓ Installed binary    → $BIN"
elif [[ -n "$APPIMAGE" ]]; then
    install -m755 "$APPIMAGE" "$BIN"
    echo "✓ Installed AppImage  → $BIN"
else
    echo "✗ No build found. Build first:  npm run tauri:build   (or: cargo build --release -p poe2-overlay)" >&2
    exit 1
fi
[[ -f "$REPO/src-tauri/icons/128x128.png" ]] && install -m644 "$REPO/src-tauri/icons/128x128.png" "$ICON_DIR/$APPID.png"

# 2. Action launchers (forward a flag to the already-running warm instance and exit)
#    + the autostart entry (the warm instance itself — no flag).
cat > "$APP_DIR/poe2-overlay-pricecheck.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=PoE2 Overlay Price Check
Comment=Trigger a PoE2 price check (forwards --price-check to the running overlay)
Exec="$BIN" --price-check
Icon=$APPID
Terminal=false
NoDisplay=true
EOF
cat > "$APP_DIR/poe2-overlay-hide.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=PoE2 Overlay Hide
Comment=Hide the PoE2 overlay (panic dismiss; forwards --hide to the running overlay)
Exec="$BIN" --hide
Icon=$APPID
Terminal=false
NoDisplay=true
EOF
cat > "$APP_DIR/poe2-overlay-runes.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=PoE2 Overlay Rune Prices
Comment=Show the rune price sheet (forwards --runes to the running overlay)
Exec="$BIN" --runes
Icon=$APPID
Terminal=false
NoDisplay=true
EOF
cat > "$AUTOSTART_DIR/poe2-overlay.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=PoE2 Overlay
Comment=Warm PoE2 trade overlay (starts hidden; price-check shows it via Ctrl+Alt+D)
Exec=$WAYLAND_ENV "$BIN"
Icon=$APPID
Terminal=false
X-GNOME-Autostart-enabled=true
EOF
echo "✓ Wrote launchers + autostart in $APP_DIR / $AUTOSTART_DIR"

# 3. Register the KWin global shortcuts (the .desktop files own the keys, per ADR-0002).
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-pricecheck.desktop --key _launch "$PRICE_KEY"
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-hide.desktop --key _launch "$HIDE_KEY"
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-runes.desktop --key _launch "$RUNES_KEY"
echo "✓ Registered shortcuts: price-check=$PRICE_KEY  hide=$HIDE_KEY  runes=$RUNES_KEY"

# The T8 regex cheat-sheet is disabled (its old Ctrl+Alt+F key now opens the rune sheet) —
# clean up a prior install's launcher + shortcut (idempotent; no-op if never installed).
rm -f "$APP_DIR/poe2-overlay-regex.desktop"
kwriteconfig6 --file kglobalshortcutsrc --group services --group poe2-overlay-regex.desktop --key _launch --delete 2>/dev/null || true

# 4. /dev/uinput access (needed for the Ctrl+C synth; ADR-0002 — no `input` group on
#    this machine, the session ACL grants it). Only print the privileged fix if missing.
if [[ -w /dev/uinput ]]; then
    echo "✓ /dev/uinput is writable (session ACL) — item-copy synth ready."
else
    echo "! /dev/uinput is NOT writable. Install the udev rule yourself (needs sudo):"
    echo "    sudo install -m644 $REPO/packaging/99-poe2-overlay-uinput.rules /etc/udev/rules.d/"
    echo "    sudo udevadm control --reload-rules && sudo udevadm trigger"
    echo "  (or: sudo usermod -aG input \$USER, then re-login)"
fi

# 5. Fontin — the PoE UI typeface the overlay's theme prefers (free for personal and
#    commercial USE, but not redistributable, so it can't ship in this repo; fetched
#    best-effort from the author's site instead). The CSS falls back to system serifs
#    when absent, so failure here is cosmetic only.
FONT_DIR="$HOME/.local/share/fonts/fontin"
if [[ -f "$FONT_DIR/Fontin-Regular.ttf" ]]; then
    echo "✓ Fontin already installed → $FONT_DIR"
elif command -v curl >/dev/null && command -v unzip >/dev/null; then
    if curl -fsL --max-time 30 -o /tmp/fontin_pc.zip "https://www.exljbris.com/dl/fontin2_pc.zip" 2>/dev/null; then
        mkdir -p "$FONT_DIR"
        unzip -o -q /tmp/fontin_pc.zip -d "$FONT_DIR" && fc-cache -f "$FONT_DIR" >/dev/null
        rm -f /tmp/fontin_pc.zip
        echo "✓ Installed Fontin (exljbris, free) → $FONT_DIR"
    else
        echo "! Could not fetch Fontin (offline?) — overlay falls back to system serifs."
    fi
else
    echo "! curl/unzip missing — skipping Fontin; overlay falls back to system serifs."
fi

echo
echo "Almost done. Two things KDE needs:"
echo "  1. Log out and back in (or restart the session) so kglobalacceld picks up the"
echo "     new shortcuts AND the overlay autostarts warm. The shortcuts will NOT fire"
echo "     until kglobalacceld reloads — a fresh login is the reliable way."
echo "  2. Start it now without relogging (optional):  $WAYLAND_ENV $BIN &"
echo
echo "Then in-game: hover an item + $PRICE_KEY to price-check; $RUNES_KEY for rune prices; $HIDE_KEY to hide."
