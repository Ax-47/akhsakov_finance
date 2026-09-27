#!/usr/bin/env bash
# Installs Akhsakov Finance for the current user on Linux (x86_64) and macOS.
#
#   curl -fsSL https://raw.githubusercontent.com/Ax-47/akhsakov_finance/main/installer.sh | bash
#
# It installs the desktop app from the latest GitHub release, and the
# `akhsakov-finance` command:
#
#   akhsakov-finance                open the app
#   akhsakov-finance run server     run only the server (--lan for phones)
#   akhsakov-finance update         install the latest release
#   akhsakov-finance uninstall      remove the app (--purge: and your data)
#
# Windows has installer.ps1. Run with --help for the options.

# The braces make bash read the whole file before running any of it, so a
# download cut short by `curl | bash` runs nothing.
{
set -euo pipefail

REPO="Ax-47/akhsakov_finance"
APP_NAME="Akhsakov Finance"
INSTALLER_URL="https://raw.githubusercontent.com/$REPO/main/installer.sh"
# Ends the line this installer adds to your shell's startup file.
MARK="# added by the akhsakov-finance installer"

case "$(uname -s)" in
Linux) OS=linux ;;
Darwin) OS=macos ;;
*)
    echo "error: this installer is for Linux and macOS. On Windows use installer.ps1 (see the README)." >&2
    exit 1
    ;;
esac

BIN_DIR="$HOME/.local/bin"
CLI="$BIN_DIR/akhsakov-finance"
if [ "$OS" = linux ]; then
    DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
    # Program and data both live here. WebKit keeps the app's offline copy of
    # pages and prices here too, because the app runs as `akhsakov-finance`.
    DATA_DIR="$DATA_HOME/akhsakov-finance"
    APP="$DATA_DIR/app"
    SERVER_BIN="$APP/usr/bin/akhsakov-finance-server"
    DESKTOP_FILE="$DATA_HOME/applications/akhsakov-finance.desktop"
else
    # The folder the app itself uses for its data (src/local_server.rs).
    DATA_DIR="$HOME/Library/Application Support/akhsakov-finance"
    APP="$HOME/Applications/AkhsakovFinance.app"
    SERVER_BIN="$APP/Contents/MacOS/akhsakov-finance-server"
    DESKTOP_FILE=""
fi

usage() {
    cat <<EOF
Install $APP_NAME for this user.

Usage: installer.sh [options]

  --version TAG        install this release instead of the latest one
  --from FILE          install a local build: the .AppImage (Linux) or the
                       zipped .app (macOS)
  --server FILE        with --from: the server binary, for builds that don't
                       carry one
  --bundled-libs       Linux: use the libraries inside the AppImage even when
                       the system has what the app needs
  --no-modify-path     don't add ~/.local/bin to your shell's PATH
  --uninstall          remove the app and keep your data
  --purge              remove the app and your data
  -h, --help           show this help

Installs:
  $APP
  $CLI
Your data: $DATA_DIR
EOF
}

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

VERSION=""
FROM=""
FROM_SERVER=""
BUNDLED_LIBS=0
MODIFY_PATH=1
ACTION=install
while [ $# -gt 0 ]; do
    case "$1" in
    --version)
        [ $# -ge 2 ] || die "--version needs a release tag"
        VERSION="$2"
        shift
        ;;
    --from)
        [ $# -ge 2 ] || die "--from needs a file"
        FROM="$2"
        shift
        ;;
    --server)
        [ $# -ge 2 ] || die "--server needs a file"
        FROM_SERVER="$2"
        shift
        ;;
    --bundled-libs) BUNDLED_LIBS=1 ;;
    --no-modify-path) MODIFY_PATH=0 ;;
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

# --- The akhsakov-finance command --------------------------------------------
# Written with this install's folders filled in. Uninstalling runs its
# `uninstall`, so the installer and the command remove the same things.

write_cli() {
    {
        echo '#!/usr/bin/env bash'
        echo "# The $APP_NAME command. Written by installer.sh; \`akhsakov-finance update\` replaces it."
        printf 'OS=%q\n' "$OS"
        printf 'DATA_DIR=%q\n' "$DATA_DIR"
        printf 'APP=%q\n' "$APP"
        printf 'SERVER_BIN=%q\n' "$SERVER_BIN"
        printf 'CLI=%q\n' "$CLI"
        printf 'DESKTOP_FILE=%q\n' "$DESKTOP_FILE"
        printf 'INSTALLER_URL=%q\n' "$INSTALLER_URL"
        printf 'MARK=%q\n' "$MARK"
        cat <<'CLI'
set -u
APP_NAME="Akhsakov Finance"

usage() {
    cat <<EOF
Usage: akhsakov-finance [command]

  run [app]               open the app (the default)
  run server [options]    run only the server, e.g. for the phone app
      --lan               let phones and computers on your network connect
      --port N            listen on port N (default 8080, which the app uses)
  update [--version TAG]  install the latest release
  uninstall [--purge]     remove the app; --purge also deletes your data
  version                 show the installed version

Your data: $DATA_DIR
EOF
}

say() { printf '%s\n' "$*"; }
die() {
    printf 'akhsakov-finance: %s\n' "$*" >&2
    # Opened from the app menu, there's no terminal to show this in.
    if [ ! -t 2 ] && command -v notify-send >/dev/null; then
        notify-send "$APP_NAME" "$*" 2>/dev/null || true
    fi
    exit 1
}

port_open() { (: </dev/tcp/127.0.0.1/"$1") 2>/dev/null; }

# True once the app's server answers on port $1 (it reports the sign-in
# state here).
server_ready() {
    if command -v curl >/dev/null; then
        curl -fs -m 2 -X POST -H 'Content-Type: application/json' -d '{}' \
            "http://127.0.0.1:$1/api/auth/status" >/dev/null 2>&1
    else
        port_open "$1"
    fi
}

# True while nobody has an account, so anyone who can reach the server gets in.
needs_account() {
    curl -fs -m 2 -X POST -H 'Content-Type: application/json' -d '{}' \
        "http://127.0.0.1:$1/api/auth/status" 2>/dev/null | grep -q '"needs_setup":true'
}

SERVER_PID=""
CLIENT_PID=""
cleanup() {
    [ -n "$CLIENT_PID" ] && kill "$CLIENT_PID" 2>/dev/null
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
    return 0
}

# Starts the server in the background on address $1, port $2.
start_server() {
    [ -x "$SERVER_BIN" ] || die "$SERVER_BIN is missing. Reinstall with: akhsakov-finance update"
    # The server serves static files from public/ and won't start without it.
    mkdir -p "$DATA_DIR/public"
    export AKHSAKOV_DB="${AKHSAKOV_DB:-$DATA_DIR/akhsakov_finance.db}"
    IP="$1" PORT="$2" DIOXUS_PUBLIC_PATH="$DATA_DIR/public" "$SERVER_BIN" &
    SERVER_PID=$!
}

# Waits for the server on port $1; the first start creates the database.
wait_for_server() {
    for _ in $(seq 150); do
        server_ready "$1" && return 0
        kill -0 "$SERVER_PID" 2>/dev/null || {
            SERVER_PID=""
            return 1
        }
        sleep 0.2
    done
    return 1
}

run_app() {
    if [ "$OS" = macos ]; then
        # The app starts its own server.
        [ -d "$APP" ] || die "$APP is missing. Reinstall with: akhsakov-finance update"
        exec open "$APP"
    fi
    [ -x "$APP/AppRun" ] || die "$APP is missing. Reinstall with: akhsakov-finance update"
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    # Start the server unless one already answers, and stop it on exit.
    if ! server_ready 8080; then
        port_open 8080 && die "port 8080 is taken by another program. Close it and try again."
        log="$DATA_DIR/server.log"
        mkdir -p "$DATA_DIR"
        [ -f "$log" ] && mv -f "$log" "$log.old"
        start_server 127.0.0.1 8080 >"$log" 2>&1
        wait_for_server 8080 || die "the server didn't start. See $log"
    fi
    # Named akhsakov-finance so the window's class (StartupWMClass, or the
    # Wayland app id) matches akhsakov-finance.desktop and gets its icon.
    (exec -a akhsakov-finance "$APP/AppRun" "$@") &
    CLIENT_PID=$!
    wait "$CLIENT_PID"
    status=$?
    CLIENT_PID=""
    exit "$status"
}

# This computer's addresses on the local network.
lan_addresses() {
    if command -v ip >/dev/null; then
        ip -4 -o addr show scope global 2>/dev/null | awk '{ sub(/\/.*/, "", $4); print $4 }'
    elif command -v ifconfig >/dev/null; then
        ifconfig 2>/dev/null | awk '$1 == "inet" && $2 != "127.0.0.1" { sub(/^addr:/, "", $2); print $2 }'
    else
        hostname -I 2>/dev/null | tr ' ' '\n' | grep -F . || true
    fi
}

run_server() {
    local ip=127.0.0.1 port=8080
    while [ $# -gt 0 ]; do
        case "$1" in
        --lan) ip=0.0.0.0 ;;
        --port)
            [ $# -ge 2 ] || die "--port needs a number"
            port="$2"
            shift
            ;;
        --port=*) port="${1#*=}" ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) die "unknown option for run server: $1" ;;
        esac
        shift
    done
    case "$port" in '' | *[!0-9]*) die "--port needs a number" ;; esac
    { [ "$port" -ge 1 ] && [ "$port" -le 65535 ]; } || die "--port needs a number from 1 to 65535"
    if server_ready "$port" && [ "$port" = 8080 ]; then
        die "a server is already running on port 8080, probably the app's own. Close the app, run this
again, then open the app: it uses this server while it runs."
    fi
    server_ready "$port" && die "a server is already running on port $port."
    port_open "$port" && die "port $port is taken by another program. Pick another with --port."

    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM HUP
    say "Starting the $APP_NAME server..."
    # Its start-up lists every endpoint; leave those lines out.
    start_server "$ip" "$port" > >(grep --line-buffered -v -e 'Registering: ' -e 'No index.html found') 2>&1
    wait_for_server "$port" || die "the server didn't start (see the messages above)."
    say ""
    say "✓ The server is running. Data: $AKHSAKOV_DB"
    if [ "$ip" = 0.0.0.0 ]; then
        say "  In the phone app, enter one of these addresses:"
        addresses="$(lan_addresses)"
        [ -n "$addresses" ] || say "    http://<this computer's IP address>:$port"
        for address in $addresses; do
            say "    http://$address:$port"
        done
        if needs_account "$port"; then
            say "  Anyone on your network can open your data until you create an account"
            say "  (Settings → Security in the app)."
        fi
    else
        say "  Only this computer can connect: http://127.0.0.1:$port"
        say "  For the phone app, run: akhsakov-finance run server --lan"
    fi
    say "  Press Ctrl+C to stop."
    wait "$SERVER_PID"
}

update() {
    command -v curl >/dev/null || die "curl is required."
    curl -fsSL "${AKHSAKOV_INSTALLER_URL:-$INSTALLER_URL}" | bash -s -- "$@"
}

# Takes out the PATH line the installer added to shell startup files.
remove_path_entry() {
    local file
    for file in "${ZDOTDIR:-$HOME}/.zshrc" "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile"; do
        grep -qF "$MARK" "$file" 2>/dev/null || continue
        grep -vF "$MARK" "$file" >"$file.akhsakov-finance" || true
        cat "$file.akhsakov-finance" >"$file"
        rm -f "$file.akhsakov-finance"
    done
    rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/akhsakov-finance.fish"
}

uninstall() {
    local purge=0
    case "${1:-}" in
    --purge) purge=1 ;;
    "") ;;
    *) die "unknown option for uninstall: $1" ;;
    esac
    rm -rf "$APP" "$APP.old"
    rm -f "$DATA_DIR/VERSION"
    if [ "$OS" = linux ]; then
        rm -f "$DATA_DIR/akhsakov-finance.png" "$DESKTOP_FILE"
        # Left by earlier versions of the installer.
        rm -rf "$DATA_DIR/server" "$DATA_DIR/server.old"
        rm -f "$DATA_DIR/akhsakov-finance.AppImage" "$DATA_DIR/akhsakov-finance-server"
        update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true
    fi
    remove_path_entry
    rm -f "$CLI"
    if [ "$purge" = 1 ]; then
        rm -rf "$DATA_DIR"
        if [ "$OS" = linux ]; then
            rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/akhsakov-finance" \
                "${XDG_CACHE_HOME:-$HOME/.cache}/akhsakov-finance"
        else
            rm -rf "$HOME/Library/WebKit/com.akhsakov.finance" "$HOME/Library/Caches/com.akhsakov.finance"
        fi
        say "✓ $APP_NAME and its data were removed."
    else
        rmdir "$DATA_DIR" 2>/dev/null || true
        say "✓ $APP_NAME was removed."
        if [ -d "$DATA_DIR" ]; then
            say "  Your data is still in $DATA_DIR"
            say "  (to delete it too: curl -fsSL $INSTALLER_URL | bash -s -- --purge)"
        fi
    fi
}

cmd="${1:-run}"
[ $# -gt 0 ] && shift
case "$cmd" in
run)
    what="${1:-app}"
    [ $# -gt 0 ] && shift
    case "$what" in
    app) run_app "$@" ;;
    server) run_server "$@" ;;
    *) die "run what? Use: akhsakov-finance run [app|server]" ;;
    esac
    ;;
update) update "$@" ;;
uninstall) uninstall "$@" ;;
version | --version) cat "$DATA_DIR/VERSION" 2>/dev/null || die "no version recorded" ;;
help | -h | --help) usage ;;
*)
    usage >&2
    exit 1
    ;;
esac
CLI
    } >"$1"
    chmod +x "$1"
}

if [ "$ACTION" != install ]; then
    uninstaller="$(mktemp)"
    write_cli "$uninstaller"
    if [ "$ACTION" = purge ]; then
        bash "$uninstaller" uninstall --purge
    else
        bash "$uninstaller" uninstall
    fi
    rm -f "$uninstaller"
    exit 0
fi

# --- Checks -----------------------------------------------------------------

if [ "$OS" = linux ]; then
    case "$(uname -m)" in
    x86_64 | amd64) ARCH=x86_64 ;;
    *) die "Linux releases are built for x86_64 only; this machine is $(uname -m)." ;;
    esac
else
    ARCH="$(uname -m)"
    # A Terminal running under Rosetta reports x86_64 on Apple silicon.
    [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ] && ARCH=arm64
    case "$ARCH" in
    arm64 | aarch64) ARCH=aarch64 ;;
    x86_64) ;;
    *) die "unsupported Mac: $ARCH" ;;
    esac
fi
[ "$(id -u)" != 0 ] || warn "installing for root. Run it without sudo to install for your own user."
if [ -n "$FROM" ]; then
    [ -f "$FROM" ] || die "no such file: $FROM"
    [ -z "$FROM_SERVER" ] || [ -f "$FROM_SERVER" ] || die "no such file: $FROM_SERVER"
    [ -z "$VERSION" ] || die "--version can't be used with --from."
else
    [ -z "$FROM_SERVER" ] || die "--server goes with --from."
    command -v curl >/dev/null || die "curl is required."
fi

mkdir -p "$DATA_DIR"
# Work next to the install folder: /tmp is often noexec on Linux (the
# AppImage has to run to unpack itself) and on another filesystem.
TMP="$(mktemp -d "$DATA_DIR/.install.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT

sha256() {
    if command -v sha256sum >/dev/null; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# --- Find the release --------------------------------------------------------

URLS=""
SUMS=""
if [ -z "$FROM" ]; then
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
    sums_url="$(printf '%s\n' "$URLS" | grep '/SHA256SUMS$' || true)"
    if [ -n "$sums_url" ]; then
        curl -fsSL -o "$TMP/SHA256SUMS" "$sums_url" || die "download failed: $sums_url"
        SUMS="$TMP/SHA256SUMS"
    fi
else
    VERSION="local"
fi

# The download URL of the first asset whose name matches one of the patterns.
asset_url() {
    local pattern url
    for pattern in "$@"; do
        for url in $URLS; do
            # shellcheck disable=SC2254 # the patterns are globs
            case "${url##*/}" in
            $pattern)
                echo "$url"
                return 0
                ;;
            esac
        done
    done
    return 1
}

# Downloads asset URL $1 to $2 and checks it against SHA256SUMS when the
# release has one.
fetch() {
    local name="${1##*/}" expected
    say "Downloading $name..."
    curl -fL --retry 3 --progress-bar -o "$2" "$1" || die "download failed: $1"
    [ -n "$SUMS" ] || return 0
    expected="$(awk -v name="$name" '$2 == name || $2 == "*" name { print $1 }' "$SUMS")"
    [ -n "$expected" ] || die "$name isn't listed in the release's SHA256SUMS."
    [ "$(sha256 "$2")" = "$expected" ] || die "$name doesn't match its checksum; the download may be damaged. Try again."
}

# The server binary for a build that doesn't carry one (releases before the
# app started its own server), copied to $1.
fetch_server() {
    if [ -n "$FROM_SERVER" ]; then
        cp "$FROM_SERVER" "$1"
    elif [ -n "$URLS" ]; then
        local url patterns=("akhsakov-finance-server-$OS-$ARCH" "*server*$OS*$ARCH*")
        # Release 1 had a single Linux server named just `server`.
        [ "$OS" = linux ] && patterns+=(server)
        url="$(asset_url "${patterns[@]}")" || die "release $VERSION has no server for $OS $ARCH."
        fetch "$url" "$1"
    else
        die "this build has no server inside; pass it with --server FILE."
    fi
    chmod +x "$1"
}

# --- Linux: unpack the AppImage ------------------------------------------------
# Unpacked, the app starts faster and doesn't need FUSE (libfuse2), which
# many distributions no longer install.

install_linux() {
    local url
    if [ -n "$FROM" ]; then
        cp "$FROM" "$TMP/app.AppImage"
    else
        url="$(asset_url "akhsakov-finance-linux-$ARCH.AppImage" "*$ARCH*.AppImage")" ||
            die "release $VERSION has no $ARCH AppImage."
        fetch "$url" "$TMP/app.AppImage"
    fi
    [ "$(head -c 4 "$TMP/app.AppImage" | od -An -c | tr -d ' ')" = '177ELF' ] ||
        die "the AppImage isn't a Linux program."
    chmod +x "$TMP/app.AppImage"
    say "Unpacking..."
    (cd "$TMP" && ./app.AppImage --appimage-extract </dev/null >/dev/null) ||
        die "couldn't unpack the AppImage."
    APP_NEW="$TMP/squashfs-root"
    [ -e "$APP_NEW/AppRun" ] || die "the AppImage has no AppRun."
    [ -f "$APP_NEW/usr/bin/akhsakov-finance-server" ] ||
        fetch_server "$APP_NEW/usr/bin/akhsakov-finance-server"
    choose_libraries

    # Swap the new files in; a running copy keeps its already-open files.
    rm -rf "$APP.old"
    [ -e "$APP" ] && mv "$APP" "$APP.old"
    mv "$APP_NEW" "$APP"
    rm -rf "$APP.old"
    # Left by earlier versions of the installer.
    rm -rf "$DATA_DIR/server"
    rm -f "$DATA_DIR/akhsakov-finance.AppImage" "$DATA_DIR/akhsakov-finance-server"

    ICON=""
    if cp -L "$APP/.DirIcon" "$DATA_DIR/akhsakov-finance.png" 2>/dev/null; then
        ICON="$DATA_DIR/akhsakov-finance.png"
    fi
    mkdir -p "$(dirname "$DESKTOP_FILE")"
    cat >"$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Personal investment portfolio tracker
Exec="$CLI"
${ICON:+Icon=$ICON
}Terminal=false
Categories=Office;Finance;
Keywords=portfolio;stocks;investment;finance;
StartupWMClass=akhsakov-finance
EOF
    update-desktop-database "$(dirname "$DESKTOP_FILE")" >/dev/null 2>&1 || true
}

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

# The AppImage carries the libraries of the machine it was built on, but
# WebKit starts its helper processes (WebKitWebProcess, ...) from the system,
# and both sides must be the same WebKit version. So the app uses the
# system's libraries, plus bundled ones only for what the system lacks
# (often libxdo), and all the bundled ones only when that doesn't work.
choose_libraries() {
    APP_BIN="$(readlink -f "$APP_NEW/AppRun")"
    local lib_dir="$APP_NEW/usr/lib" so moved missing
    LIBS=bundled
    KEPT=0
    SYSTEM_MISSING=""
    if [ "$BUNDLED_LIBS" = 0 ] && [ -d "$lib_dir" ] && command -v ldd >/dev/null; then
        mkdir "$TMP/bundled-libs"
        find "$lib_dir" -maxdepth 1 -name '*.so*' ! -type d -exec mv {} "$TMP/bundled-libs/" \;
        while :; do
            moved=0
            for so in $(ldd "$APP_BIN" 2>/dev/null | awk '$2 == "=>" && $3 == "not" { print $1 }'); do
                case "$so" in libwebkit2gtk* | libjavascriptcoregtk*) continue ;; esac
                if [ -e "$TMP/bundled-libs/$so" ]; then
                    mv "$TMP/bundled-libs/$so" "$lib_dir/"
                    moved=1
                    KEPT=$((KEPT + 1))
                fi
            done
            [ "$moved" = 1 ] || break
        done
        SYSTEM_MISSING="$(missing_libs)"
        if [ -z "$SYSTEM_MISSING" ]; then
            LIBS=system
        else
            find "$TMP/bundled-libs" -mindepth 1 -maxdepth 1 -exec mv {} "$lib_dir/" \;
        fi
    fi
    missing="$(missing_libs)"
    if [ -n "$missing" ]; then
        {
            say "The app can't run on this system. Missing or too old:"
            printf '%s\n' "${SYSTEM_MISSING:-$missing}" | head -n 8 | sed 's/^/  /'
        } >&2
        die "install WebKitGTK 4.1 and xdotool (libxdo), then run the installer again:
  Arch:          sudo pacman -S webkit2gtk-4.1 xdotool
  Debian/Ubuntu: sudo apt install libwebkit2gtk-4.1-0 libxdo3
  Fedora:        sudo dnf install webkit2gtk4.1 libxdo
If they're installed and this still fails, the release was built on a newer
system than this one. Building from source works everywhere (see README)."
    fi
    if [ "$LIBS" = bundled ]; then
        local helpers
        helpers="$(grep -a -o -m 1 '/usr/lib[^[:cntrl:] ]*/webkit2gtk-4\.1' \
            "$lib_dir/libwebkit2gtk-4.1.so.0" 2>/dev/null | head -n 1 || true)"
        if [ -n "$helpers" ] && [ ! -x "$helpers/WebKitWebProcess" ]; then
            warn "$helpers/WebKitWebProcess is missing, so the window may stay blank.
Installing WebKitGTK 4.1 from your package manager usually fixes it."
        fi
    fi
}

# --- macOS: the zipped .app ----------------------------------------------------

install_macos() {
    local url app
    if [ -n "$FROM" ]; then
        cp "$FROM" "$TMP/app.zip"
    else
        url="$(asset_url "akhsakov-finance-macos-$ARCH.zip" "*macos*$ARCH*.zip")" ||
            die "release $VERSION has no build for this Mac ($ARCH)."
        fetch "$url" "$TMP/app.zip"
    fi
    say "Unpacking..."
    ditto -x -k "$TMP/app.zip" "$TMP/unzipped" || die "couldn't unpack $(basename "${FROM:-app.zip}")."
    app="$(find "$TMP/unzipped" -maxdepth 2 -name '*.app' -type d | head -n 1)"
    [ -n "$app" ] || die "there's no .app in the download."
    [ -f "$app/Contents/MacOS/akhsakov-finance-server" ] ||
        fetch_server "$app/Contents/MacOS/akhsakov-finance-server"
    # Downloads from a browser are quarantined; curl's aren't, but a file
    # passed with --from may be.
    xattr -dr com.apple.quarantine "$app" 2>/dev/null || true

    mkdir -p "$(dirname "$APP")"
    rm -rf "$APP.old"
    [ -e "$APP" ] && mv "$APP" "$APP.old"
    mv "$app" "$APP"
    rm -rf "$APP.old"
}

# --- PATH ---------------------------------------------------------------------

# Adds ~/.local/bin to PATH in the shell's startup file, when it's missing.
add_path_entry() {
    PATH_FILE=""
    case ":$PATH:" in *":$BIN_DIR:"*) return 0 ;; esac
    [ "$MODIFY_PATH" = 1 ] || return 0
    case "$(basename "${SHELL:-}")" in
    zsh) PATH_FILE="${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) if [ "$OS" = macos ]; then PATH_FILE="$HOME/.bash_profile"; else PATH_FILE="$HOME/.bashrc"; fi ;;
    fish)
        PATH_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/akhsakov-finance.fish"
        mkdir -p "$(dirname "$PATH_FILE")"
        printf "fish_add_path -g '%s' %s\n" "$BIN_DIR" "$MARK" >"$PATH_FILE"
        return 0
        ;;
    *) return 0 ;;
    esac
    grep -qF "$MARK" "$PATH_FILE" 2>/dev/null && return 0
    # Start on a line of its own.
    if [ -s "$PATH_FILE" ] && [ -n "$(tail -c 1 "$PATH_FILE")" ]; then echo >>"$PATH_FILE"; fi
    # shellcheck disable=SC2016 # $HOME and $PATH expand when the shell starts
    printf 'export PATH="$HOME/.local/bin:$PATH" %s\n' "$MARK" >>"$PATH_FILE"
}

# --- Install ------------------------------------------------------------------

if [ "$OS" = linux ]; then
    install_linux
else
    install_macos
fi

mkdir -p "$BIN_DIR"
write_cli "$CLI"
add_path_entry
PREVIOUS="$(cat "$DATA_DIR/VERSION" 2>/dev/null || true)"
printf '%s\n' "$VERSION" >"$DATA_DIR/VERSION"

say
if [ -n "$PREVIOUS" ] && [ "$PREVIOUS" != "$VERSION" ]; then
    say "✓ $APP_NAME updated from $PREVIOUS to $VERSION."
else
    say "✓ $APP_NAME $VERSION installed."
fi
if [ "$OS" = linux ]; then
    if [ "$LIBS" = system ] && [ "$KEPT" -gt 0 ]; then
        say "  Using this system's WebKitGTK, and $KEPT libraries from the AppImage."
    elif [ "$LIBS" = system ]; then
        say "  Using this system's WebKitGTK."
    else
        say "  Using the libraries bundled in the AppImage."
    fi
    say "  Open it from your app menu, or run: akhsakov-finance"
else
    say "  Open it from Launchpad or ~/Applications, or run: akhsakov-finance"
fi
say "  For the phone app: akhsakov-finance run server --lan"
say "  Your data: $DATA_DIR"
if [ -n "$PATH_FILE" ]; then
    say "  Added ~/.local/bin to PATH in $PATH_FILE; open a new terminal to use the command."
else
    case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "  Note: $BIN_DIR isn't on your PATH, so the command needs its full path." ;;
    esac
fi
exit 0
}
