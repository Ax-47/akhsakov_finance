#!/usr/bin/env bash
# Installs the Akhsakov Finance desktop app for the current user (Linux x86_64).
#
#   curl -fsSL https://raw.githubusercontent.com/Ax-47/akhsakov_finance/main/installer.sh | bash
#
# The desktop client talks to the app's server on 127.0.0.1:8080, so both
# come from the same GitHub release: the client's AppImage and the `server`
# binary. The installed `akhsakov-finance` command starts the server (unless
# one is already running), opens the app, and stops the server when the
# window closes. Run with --help for the options.

# The braces make bash read the whole file before running any of it, so a
# download cut short by `curl | bash` runs nothing.
{
set -euo pipefail

REPO="Ax-47/akhsakov_finance"
APP_NAME="Akhsakov Finance"

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
# Program and data both live here. WebKit also keeps the app's offline copy of
# pages and prices here, because the app runs as `akhsakov-finance`.
APP_DIR="$DATA_HOME/akhsakov-finance"
BIN_DIR="$HOME/.local/bin"
LAUNCHER="$BIN_DIR/akhsakov-finance"
DESKTOP_FILE="$DATA_HOME/applications/akhsakov-finance.desktop"

usage() {
    cat <<EOF
Install $APP_NAME for this user.

Usage: installer.sh [options]

  --version TAG        install this release instead of the latest one
  --appimage FILE      install from a local AppImage (use with --server)
  --server FILE        install from a local server binary (use with --appimage)
  --bundled-libs       use the libraries inside the AppImage even when the
                       system has everything the app needs
  --uninstall          remove the app and keep your data
  --purge              remove the app and your data (akhsakov_finance.db
                       and the offline cache)
  -h, --help           show this help

Files:
  $APP_DIR    app, server and data
  $LAUNCHER
  $DESKTOP_FILE
EOF
}

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

VERSION=""
LOCAL_APPIMAGE=""
LOCAL_SERVER=""
BUNDLED_LIBS=0
ACTION=install
while [ $# -gt 0 ]; do
    case "$1" in
    --version)
        [ $# -ge 2 ] || die "--version needs a release tag"
        VERSION="$2"
        shift
        ;;
    --appimage)
        [ $# -ge 2 ] || die "--appimage needs a file"
        LOCAL_APPIMAGE="$2"
        shift
        ;;
    --server)
        [ $# -ge 2 ] || die "--server needs a file"
        LOCAL_SERVER="$2"
        shift
        ;;
    --bundled-libs) BUNDLED_LIBS=1 ;;
    --uninstall) ACTION=uninstall ;;
    --purge) ACTION=purge ;;
    -h | --help)
        usage
        exit 0
        ;;
    *) die "unknown option: $1 (see --help)" ;;
    esac
    shift
done

remove_app() {
    rm -rf "$APP_DIR/app" "$APP_DIR/server"
    rm -f "$APP_DIR/akhsakov-finance.png" "$APP_DIR/VERSION"
    # Left by the first version of this installer.
    rm -f "$APP_DIR/akhsakov-finance.AppImage" "$APP_DIR/akhsakov-finance-server"
    rm -f "$LAUNCHER" "$DESKTOP_FILE"
    update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true
}

if [ "$ACTION" != install ]; then
    remove_app
    if [ "$ACTION" = purge ]; then
        rm -rf "$APP_DIR" "${XDG_STATE_HOME:-$HOME/.local/state}/akhsakov-finance" \
            "${XDG_CACHE_HOME:-$HOME/.cache}/akhsakov-finance"
        say "✓ $APP_NAME and its data were removed."
    else
        rmdir "$APP_DIR" 2>/dev/null || true
        say "✓ $APP_NAME was removed."
        [ -d "$APP_DIR" ] && say "  Your data is still in $APP_DIR (run with --purge to delete it)."
    fi
    exit 0
fi

[ "$(uname -s)" = Linux ] || die "this installer supports Linux only."
[ "$(id -u)" != 0 ] || warn "installing for root. Run it without sudo to install for your own user."
case "$(uname -m)" in
x86_64 | amd64) ;;
*) die "releases are built for x86_64 only; this machine is $(uname -m)." ;;
esac
if [ -n "$LOCAL_APPIMAGE$LOCAL_SERVER" ]; then
    [ -n "$LOCAL_APPIMAGE" ] && [ -n "$LOCAL_SERVER" ] ||
        die "--appimage and --server go together."
    [ -f "$LOCAL_APPIMAGE" ] || die "no such file: $LOCAL_APPIMAGE"
    [ -f "$LOCAL_SERVER" ] || die "no such file: $LOCAL_SERVER"
    [ -z "$VERSION" ] || die "--version can't be used with local files."
else
    command -v curl >/dev/null || die "curl is required."
fi

# Work next to the install folder: /tmp is often noexec (the AppImage has to
# run to unpack itself) and on another filesystem (slower moves).
mkdir -p "$DATA_HOME"
TMP="$(mktemp -d "$DATA_HOME/.akhsakov-finance-install.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

is_elf() { [ "$(head -c 4 "$1" | od -An -c | tr -d ' ')" = '177ELF' ]; }

# --- Fetch -----------------------------------------------------------------

if [ -n "$LOCAL_APPIMAGE" ]; then
    VERSION="local"
    cp "$LOCAL_APPIMAGE" "$TMP/app.AppImage"
    cp "$LOCAL_SERVER" "$TMP/server"
else
    if [ -n "$VERSION" ]; then
        API="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
        say "Looking up release $VERSION of $APP_NAME..."
    else
        API="https://api.github.com/repos/$REPO/releases/latest"
        say "Looking up the latest release of $APP_NAME..."
    fi
    RELEASE="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$API")" ||
        die "couldn't read $API (no such release, or GitHub is unreachable)."
    VERSION="$(printf '%s' "$RELEASE" | grep -o '"tag_name": *"[^"]*"' | head -n 1 | sed 's/.*"\([^"]*\)"$/\1/' || true)"
    URLS="$(printf '%s' "$RELEASE" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' || true)"

    APPIMAGE_URL=""
    SERVER_URL=""
    for url in $URLS; do
        name="${url##*/}"
        case "$name" in
        *.AppImage)
            case "$name" in
            *aarch64* | *arm64* | *armhf* | *i686* | *i386*) ;;
            *) [ -n "$APPIMAGE_URL" ] || APPIMAGE_URL="$url" ;;
            esac
            ;;
        *.aab | *.apk | *.ipa | *.sha256 | *.sig | *.asc) ;;
        *server*) [ -n "$SERVER_URL" ] || SERVER_URL="$url" ;;
        esac
    done
    [ -n "$APPIMAGE_URL" ] || die "release $VERSION has no x86_64 .AppImage."
    [ -n "$SERVER_URL" ] || die "release $VERSION has no server binary."

    say "Downloading $APP_NAME $VERSION..."
    curl -fL --retry 3 --progress-bar -o "$TMP/app.AppImage" "$APPIMAGE_URL" ||
        die "download failed: $APPIMAGE_URL"
    curl -fL --retry 3 --progress-bar -o "$TMP/server" "$SERVER_URL" ||
        die "download failed: $SERVER_URL"
fi

is_elf "$TMP/app.AppImage" || die "the AppImage isn't a Linux program."
is_elf "$TMP/server" || die "the server isn't a Linux program."
chmod +x "$TMP/app.AppImage" "$TMP/server"

# --- Unpack the AppImage ----------------------------------------------------
# Unpacked, the app starts faster and doesn't need FUSE (libfuse2), which
# many distributions no longer install.

say "Unpacking..."
(cd "$TMP" && ./app.AppImage --appimage-extract </dev/null >/dev/null) ||
    die "couldn't unpack the AppImage."
APP_NEW="$TMP/squashfs-root"
[ -e "$APP_NEW/AppRun" ] || die "the AppImage has no AppRun."
APP_BIN="$(readlink -f "$APP_NEW/AppRun")"
LIB_DIR="$APP_NEW/usr/lib"

# What the dynamic loader can't find, one item per line (empty when nothing):
# a library, a glibc version, or a symbol. The app binds every symbol at
# start-up, so a library that is present but too old counts as missing too.
missing_libs() {
    command -v ldd >/dev/null || return 0
    ldd -r "$APP_BIN" 2>&1 |
        grep -o -E "[^[:space:]/]+ => not found|version \`[^']+' not found|undefined symbol: [^[:space:],]+(, version [^[:space:]]+)?" |
        sed -E "s/ => not found$//; s/^version \`([^']+)' not found$/\1/; s/^undefined symbol: [^,]+, version //; s/^undefined symbol: /symbol /" |
        sort -u || true
}

# The AppImage carries the WebKitGTK libraries of the machine it was built on,
# but WebKit starts its helper processes (WebKitWebProcess, ...) from the
# system, and the two must be the same WebKit version. So the system's
# libraries are used when they cover everything, and the bundled ones only
# when they don't.
LIBS=bundled
SYSTEM_MISSING=""
if [ "$BUNDLED_LIBS" = 0 ] && [ -d "$LIB_DIR" ] && command -v ldd >/dev/null; then
    mkdir "$TMP/bundled-libs"
    find "$LIB_DIR" -maxdepth 1 -name '*.so*' ! -type d -exec mv {} "$TMP/bundled-libs/" \;
    SYSTEM_MISSING="$(missing_libs)"
    if [ -z "$SYSTEM_MISSING" ]; then
        LIBS=system
    else
        find "$TMP/bundled-libs" -mindepth 1 -maxdepth 1 -exec mv {} "$LIB_DIR/" \;
    fi
fi
MISSING="$(missing_libs)"
if [ -n "$MISSING" ]; then
    {
        say "The app can't run on this system. Missing or too old:"
        printf '%s\n' "${SYSTEM_MISSING:-$MISSING}" | head -n 8 | sed 's/^/  /'
    } >&2
    die "install WebKitGTK 4.1 and xdotool (libxdo), then run the installer again:
  Arch:          sudo pacman -S webkit2gtk-4.1 xdotool
  Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libxdo3
  Fedora:        sudo dnf install webkit2gtk4.1 libxdo
If they're installed and this still fails, the release was built on a newer
system than this one. Building from source works everywhere (see README)."
fi
if [ "$LIBS" = bundled ]; then
    webkit_helpers="$(grep -a -o -m 1 '/usr/lib[^[:cntrl:] ]*/webkit2gtk-4\.1' \
        "$LIB_DIR/libwebkit2gtk-4.1.so.0" 2>/dev/null | head -n 1 || true)"
    if [ -n "$webkit_helpers" ] && [ ! -x "$webkit_helpers/WebKitWebProcess" ]; then
        warn "$webkit_helpers/WebKitWebProcess is missing, so the window may stay blank.
Installing WebKitGTK 4.1 from your package manager usually fixes it."
    fi
fi

# --- Install ----------------------------------------------------------------

say "Installing into $APP_DIR..."
mkdir -p "$APP_DIR" "$BIN_DIR" "$(dirname "$DESKTOP_FILE")"

# The server serves static files from public/ next to its binary and won't
# start without that folder, even though the desktop client doesn't use it.
mkdir -p "$TMP/server-dir/public"
mv "$TMP/server" "$TMP/server-dir/akhsakov-finance-server"

cp -L "$APP_NEW/.DirIcon" "$TMP/icon.png" 2>/dev/null || true

# Swap the new files in; a running copy keeps its already-open files.
rm -rf "$APP_DIR/app.old" "$APP_DIR/server.old"
[ -e "$APP_DIR/app" ] && mv "$APP_DIR/app" "$APP_DIR/app.old"
[ -e "$APP_DIR/server" ] && mv "$APP_DIR/server" "$APP_DIR/server.old"
mv "$APP_NEW" "$APP_DIR/app"
mv "$TMP/server-dir" "$APP_DIR/server"
rm -rf "$APP_DIR/app.old" "$APP_DIR/server.old"
rm -f "$APP_DIR/akhsakov-finance.AppImage" "$APP_DIR/akhsakov-finance-server"

ICON="$APP_DIR/akhsakov-finance.png"
if [ -f "$TMP/icon.png" ]; then
    mv "$TMP/icon.png" "$ICON"
else
    ICON=""
fi
PREVIOUS="$(cat "$APP_DIR/VERSION" 2>/dev/null || true)"
printf '%s\n' "$VERSION" >"$APP_DIR/VERSION"

# The launcher. $APP_DIR is filled in now; everything escaped runs later.
cat >"$LAUNCHER" <<EOF
#!/usr/bin/env bash
# Starts $APP_NAME. Written by installer.sh; reinstalling replaces it.
#
# The app talks to its server on 127.0.0.1:8080. This starts that server
# unless one is already answering there, and stops it when the app exits.
set -u

APP_DIR="$APP_DIR"
STATE_DIR="\${XDG_STATE_HOME:-\$HOME/.local/state}/akhsakov-finance"
LOG="\$STATE_DIR/server.log"
export AKHSAKOV_DB="\${AKHSAKOV_DB:-\$APP_DIR/akhsakov_finance.db}"

fail() {
    echo "akhsakov-finance: \$*" >&2
    if command -v notify-send >/dev/null; then
        notify-send "$APP_NAME" "\$*" 2>/dev/null || true
    fi
    exit 1
}

port_open() { (: </dev/tcp/127.0.0.1/8080) 2>/dev/null; }

# True once the app's server answers (it reports the login state here).
server_ready() {
    if command -v curl >/dev/null; then
        curl -fs -m 2 -X POST -H 'Content-Type: application/json' -d '{}' \\
            http://127.0.0.1:8080/api/auth/status >/dev/null 2>&1
    else
        port_open
    fi
}

SERVER_PID=""
CLIENT_PID=""
cleanup() {
    [ -n "\$CLIENT_PID" ] && kill "\$CLIENT_PID" 2>/dev/null
    [ -n "\$SERVER_PID" ] && kill "\$SERVER_PID" 2>/dev/null
    return 0
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

if ! server_ready; then
    port_open && fail "port 8080 is taken by another program. Close it and try again."
    mkdir -p "\$STATE_DIR"
    [ -f "\$LOG" ] && mv -f "\$LOG" "\$LOG.old"
    IP=127.0.0.1 PORT=8080 "\$APP_DIR/server/akhsakov-finance-server" >"\$LOG" 2>&1 &
    SERVER_PID=\$!
    # The first start creates the database, so give it a while.
    for _ in \$(seq 150); do
        server_ready && break
        kill -0 "\$SERVER_PID" 2>/dev/null || { SERVER_PID=""; fail "the server stopped while starting. See \$LOG"; }
        sleep 0.2
    done
    server_ready || fail "the server didn't start within 30 seconds. See \$LOG"
fi

# Named after this launcher, so the window's class (StartupWMClass, or the
# Wayland app id) matches akhsakov-finance.desktop and gets its icon.
(exec -a akhsakov-finance "\$APP_DIR/app/AppRun" "\$@") &
CLIENT_PID=\$!
wait "\$CLIENT_PID"
status=\$?
CLIENT_PID=""
exit "\$status"
EOF
chmod +x "$LAUNCHER"

cat >"$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Personal investment portfolio tracker
Exec="$LAUNCHER"
${ICON:+Icon=$ICON
}Terminal=false
Categories=Office;Finance;
Keywords=portfolio;stocks;investment;finance;
StartupWMClass=akhsakov-finance
EOF
update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true

# --- Done -------------------------------------------------------------------

say
if [ -n "$PREVIOUS" ] && [ "$PREVIOUS" != "$VERSION" ]; then
    say "✓ $APP_NAME updated from $PREVIOUS to $VERSION."
else
    say "✓ $APP_NAME $VERSION installed."
fi
if [ "$LIBS" = system ]; then
    say "  Using this system's WebKitGTK."
else
    say "  Using the libraries bundled in the AppImage."
fi
say "  Open it from your app menu, or run: akhsakov-finance"
say "  Your data: $APP_DIR/akhsakov_finance.db"
case ":$PATH:" in
*":$BIN_DIR:"*) ;;
*) say "  Note: $BIN_DIR isn't on your PATH, so the command needs its full path." ;;
esac
exit 0
}
