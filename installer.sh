#!/usr/bin/env bash
set -euo pipefail

REPO="Ax-47/akhsakov_finance"
INSTALL_DIR="$HOME/.local/share/akhsakov-finance"
BIN_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"

echo "Installing Akhsakov Finance..."

mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$DESKTOP_DIR"

# Get latest release
API="https://api.github.com/repos/$REPO/releases/latest"

DOWNLOAD_URL=$(
  curl -fsSL "$API" |
    grep '"browser_download_url"' |
    grep -E 'AppImage|server' |
    cut -d '"' -f 4
)

for URL in $DOWNLOAD_URL; do
  case "$URL" in
  *.AppImage)
    curl -fL "$URL" \
      -o "$INSTALL_DIR/akhsakov-finance.AppImage"
    chmod +x "$INSTALL_DIR/akhsakov-finance.AppImage"
    ;;
  *server*)
    curl -fL "$URL" \
      -o "$INSTALL_DIR/akhsakov-finance-server"
    chmod +x "$INSTALL_DIR/akhsakov-finance-server"
    ;;
  esac
done

cat >"$BIN_DIR/akhsakov-finance" <<EOF
#!/usr/bin/env bash

INSTALL_DIR="\$HOME/.local/share/akhsakov-finance"

"\$INSTALL_DIR/akhsakov-finance-server" &
SERVER_PID=\$!

cleanup() {
    kill "\$SERVER_PID" 2>/dev/null || true
}

trap cleanup EXIT INT TERM

sleep 1

"\$INSTALL_DIR/akhsakov-finance.AppImage"
EOF

chmod +x "$BIN_DIR/akhsakov-finance"

cat >"$DESKTOP_DIR/akhsakov-finance.desktop" <<EOF
[Desktop Entry]
Name=Akhsakov Finance
Comment=Personal finance application
Exec=$BIN_DIR/akhsakov-finance
Terminal=false
Type=Application
Categories=Office;Finance;
EOF

update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

echo
echo "✓ Akhsakov Finance installed!"
echo "Run: akhsakov-finance"
