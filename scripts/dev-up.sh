#!/usr/bin/env bash
set -euo pipefail
cargo run -p api-gateway &
cargo run -p agent-runtime &
wait
