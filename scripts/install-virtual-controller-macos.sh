#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LABEL="io.millo.virtual-controller"
INSTALL_DIR="${HOME}/Library/Application Support/Millo/Virtual Controller"
LOG_DIR="${HOME}/Library/Logs/Millo"
PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"
DOMAIN="gui/$(id -u)"

cd "${ROOT}"
cargo build --release -p millo-virtual-controller
mkdir -p "${INSTALL_DIR}" "${LOG_DIR}" "$(dirname "${PLIST}")"
install -m 0755 \
  "${ROOT}/target/release/millo-virtual-controller" \
  "${INSTALL_DIR}/millo-virtual-controller"

cat > "${PLIST}.tmp" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>${LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>${INSTALL_DIR}/millo-virtual-controller</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>${LOG_DIR}/virtual-controller.log</string>
  <key>StandardErrorPath</key>
  <string>${LOG_DIR}/virtual-controller-error.log</string>
</dict>
</plist>
EOF
plutil -lint "${PLIST}.tmp" >/dev/null
mv "${PLIST}.tmp" "${PLIST}"

launchctl bootout "${DOMAIN}/${LABEL}" 2>/dev/null || true
launchctl bootstrap "${DOMAIN}" "${PLIST}"
launchctl kickstart -k "${DOMAIN}/${LABEL}"

printf 'Installed %s\n' "${INSTALL_DIR}/millo-virtual-controller"
printf 'Log: %s\n' "${LOG_DIR}/virtual-controller.log"
