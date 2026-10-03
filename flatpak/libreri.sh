#!/bin/sh
# Starts Libreri (the .deb names its program after the app).
# Tesseract's own languages are in /app/share/tessdata.
export TESSDATA_PREFIX="${TESSDATA_PREFIX:-/app/share/tessdata}"
for bin in /app/lib/libreri/*; do
  exec "$bin" "$@"
done
