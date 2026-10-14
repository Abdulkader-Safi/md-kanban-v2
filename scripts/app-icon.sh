#!/bin/sh
# Rebuilds icon.png, icon.icns and icon.ico from app-icon.svg. macOS only (iconutil).
# Needs rsvg-convert and ImageMagick: brew install librsvg imagemagick
set -e
cd "$(dirname "$0")/../crates/mdk/assets/app"
set_dir=$(mktemp -d)/icon.iconset
mkdir "$set_dir"
for s in 16 32 128 256 512; do
  rsvg-convert -w $s -h $s app-icon.svg -o "$set_dir/icon_${s}x${s}.png"
  rsvg-convert -w $((s * 2)) -h $((s * 2)) app-icon.svg -o "$set_dir/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$set_dir" -o icon.icns
cp "$set_dir/icon_512x512.png" icon.png
rsvg-convert -w 256 -h 256 app-icon.svg | magick - -define icon:auto-resize=256,128,64,48,32,16 icon.ico
