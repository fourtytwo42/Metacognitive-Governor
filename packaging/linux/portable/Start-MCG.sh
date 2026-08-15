#!/usr/bin/env sh
set -eu

bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Start the loopback control plane if another MCG daemon does not already own it.
# A duplicate process exits without replacing the running user's durable state.
"$bundle_dir/mcg-daemon" >/dev/null 2>&1 &
sleep 2
exec "$bundle_dir/mcg-desktop"
