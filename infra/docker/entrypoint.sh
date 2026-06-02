#!/bin/sh
set -e

# Backend binds on 0.0.0.0:38080 inside container; nginx proxies from port 80
export PLATFORM_HTTP_ADDR="${PLATFORM_HTTP_ADDR:-0.0.0.0:38080}"

# Start backend
trigix-platform &
BACKEND_PID=$!

# Give backend a moment to start before nginx begins proxying
sleep 1

# Start nginx in foreground
nginx -g 'daemon off;' &
NGINX_PID=$!

# Exit if either process dies
wait -n $BACKEND_PID $NGINX_PID
EXIT_CODE=$?
kill $BACKEND_PID $NGINX_PID 2>/dev/null || true
exit $EXIT_CODE
