#!/bin/bash
set -e

# Build the dev container image from the Dockerfile (standard docker build; no
# Nix required). The image targets the host architecture; multi-arch builds are
# out of scope (see issue #17).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

docker build -t craftman:latest "$REPO_ROOT"
