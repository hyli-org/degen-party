#!/bin/bash
set -e

# Start nginx in the background
nginx -g 'daemon off;' &

# Run rollup inline
RUST_LOG=debug ./rollup
