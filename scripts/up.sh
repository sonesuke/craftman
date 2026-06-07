#!/bin/bash
set -e

docker run -d \
  --name craftman \
  -v "$(pwd):/workspaces/craftman" \
  -v "${HOME}/.config/gh:/home/user/.config/gh" \
  -e Z_AI_API_KEY="${Z_AI_API_KEY}" \
  craftman:latest \
  sleep infinity
