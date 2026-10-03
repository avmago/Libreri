#!/bin/sh
# Starts Libreri (the .deb names its program after the app).
for bin in /app/lib/libreri/*; do
  exec "$bin" "$@"
done
