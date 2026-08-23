#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only

set -Eeuo pipefail

PREFIX="${PREFIX:-/usr}"
APP_ID="io.github.tihulu.SystemMonitor"

sudo rm -f \
  "$PREFIX/bin/tihulu-cosmic-system-monitor" \
  "$PREFIX/share/applications/$APP_ID.desktop" \
  "$PREFIX/share/metainfo/$APP_ID.metainfo.xml" \
  "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"

echo "Tihulu System Monitor removed."
