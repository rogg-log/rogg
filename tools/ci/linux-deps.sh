#!/usr/bin/env bash
# Installs gpui's Linux build dependencies on an Ubuntu CI runner (the -dev packages of tools/linux/Containerfile).
set -euo pipefail
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config cmake clang \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxcursor-dev libxrandr-dev libxi-dev \
  libfontconfig1-dev libfreetype6-dev libvulkan-dev libssl-dev
