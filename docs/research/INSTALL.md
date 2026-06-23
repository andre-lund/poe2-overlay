# PathofTrading — install runbook (CachyOS + KDE Wayland)

Tailored to this machine (probed 2026-06-23): `quickshell` is in `extra`,
`/dev/uinput` already grants this user rw via session ACL (so `ydotoold` runs as
**your** user — no group/udev/reboot), `pip` is **not** installed.

Eval clone lives at `~/Code/poe2-overlay/eval-pathoftrading`.

## 1. System packages (needs sudo — run yourself)

```fish
sudo pacman -S --needed quickshell ydotool wl-clipboard xclip python-pip
```
(`python-pip` only satisfies `install.sh`'s dep check. `xclip` is the clipboard
fallback `get_clipboard` needs — without it every run logs an exception and you lose
the X11-selection read path that's more reliable than `wl-paste` for XWayland/Proton
items.)

## 2. ydotoold (done — persistent user service)

Installed as `~/.config/systemd/user/ydotoold.service` (enabled + running, `Restart=always`).
Runs as your user — `/dev/uinput` is reachable via the session ACL (`user:olund:rw-`),
no root/`input`-group needed. Socket: `$XDG_RUNTIME_DIR/.ydotool_socket` (matches what
`run_pricecheck.sh` expects). Manage with `systemctl --user {status,restart,stop} ydotoold`.

> Fallback if the socket ever fails to open at boot (ACL race): `sudo usermod -aG input $USER`
> then re-login — gives `ydotoold` uinput access independent of the session ACL.

## 3. Install PathofTrading

```fish
cd ~/Code/poe2-overlay/eval-pathoftrading
./install.sh
```
At the WM prompt choose **3 (Skip / KDE)**. It creates a venv at
`~/.local/share/pathoftrading-v1.0/`, installs `requests`, and symlinks
`~/.local/bin/pathoftrading`. Ensure `~/.local/bin` is on `$PATH`.

## 4. Hotkey (done — Ctrl+Alt+D)

Registered without the GUI (Plasma 6 has no khotkeys; on Wayland `kwin_wayland`
owns `org.kde.kglobalaccel`):
- launcher `~/.local/share/applications/pathoftrading.desktop` (Exec = the symlink)
- persistent entry in `~/.config/kglobalshortcutsrc`: `[services][pathoftrading.desktop] _launch=Ctrl+Alt+D`
- registered live via the kglobalaccel D-Bus API (`doRegister` + `setShortcut`,
  Qt keycode 201326660), so it works now without a relogin.

Verify: `busctl --user call org.kde.kglobalaccel /kglobalaccel org.kde.KGlobalAccel getGlobalShortcutsByKey i 201326660`
should resolve to `pathoftrading.desktop`. To change the key, use System Settings →
Keyboard → Shortcuts (search "Path of Trading").

> KDE fires on **press**, not release. The wrapper injects a kernel Ctrl+C; if
> items fail to copy because your physical modifiers interfere, rebind to a combo
> without Ctrl (e.g. a mouse side-button or `Alt+D`).

## 5. Test in-game (the actual validation)

1. Launch PoE2 via Steam/Proton.
2. Hover an item, press the hotkey. The Quickshell panel should pop up with prices.
3. **Test both display modes** — windowed/borderless-fullscreen AND exclusive
   fullscreen. The one unverified risk is whether KWin composites the layer-shell
   overlay over *exclusive* fullscreen. If it only works in borderless, that's the
   known-good config.
4. Logs: `cat ~/.cache/pathoftrading-v1.0/script.log`

## Local tweaks applied to our copy (eval-pathoftrading → installed)

- `PathofTrading.qml`: `s(val)` scale 1.0 → **1.2** (window was too small; one-number knob).
- `run_pricecheck.sh`: `sleep 0.4` → **0.2** (backend already retries the clipboard).
- `backend.py`: added **`timeout=10`** to the GGG search POST + fetch GET (they had none,
  so a GGG stall could hang for many seconds).

Redeploy after editing the source: `cp PathofTrading.qml run_pricecheck.sh backend.py ~/.local/share/pathoftrading-v1.0/`.

## Performance note (measured 2026-06-23)

Per-run fixed overhead is only ~200ms (import ~70-110ms, 888KB stats parse ~5ms,
exchange-rates fetch ~110ms). The perceived "slow load" is the **GGG trade2 search+fetch
round-trip** — their server, the floor for any price-checker. The architectural lever is
that PathofTrading spawns a **fresh Python process per hotkey**, so there's no warm HTTP
session / DNS cache between checks. A persistent daemon (always-running backend the hotkey
pings) would cut that — which is a fork-level change, not a patch. See `RESEARCH.md`.

## Safety caveats (from audit)

- The hotkey sends Ctrl+C to **whatever window is focused** — no game-focus check.
- Don't trigger it with a secret on the clipboard — the fallback path sends
  clipboard text to GGG's trade search API.
