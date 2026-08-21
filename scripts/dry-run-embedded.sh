#!/usr/bin/env bash
# Dry-run the embedded sample's instructions via the SPEL CLI.
#
# The admin slot lives inside the consumer's prog_config account at byte
# offset 32. No admin_initialize exists (the slot is born initialized by
# the sample's own initialize), the management instructions resolve the
# embedding account, and no offset appears anywhere in a transaction.
#
# Usage:  scripts/dry-run-embedded.sh [path-to-spel-repo]
# Output: prints to stdout; CI or docs can redirect to a file.

set -uo pipefail

SPEL_REPO="${1:-$(dirname "$0")/../../spel}"
SAMPLE_SRC="$(dirname "$0")/../admin-authority-sample-embedded/src/main.rs"
PROG_ID="$(printf 'ab%.0s' {1..32})"          # placeholder, fine for dry-run
CALLER="$(printf '11%.0s' {1..32})"
NEW_ACCOUNT="$(printf '22%.0s' {1..32})"
IDL="$(mktemp --suffix .idl.json)"
trap 'rm -f "$IDL"' EXIT

echo "== Building spel CLI =="
(cd "$SPEL_REPO" && RISC0_SKIP_BUILD=1 cargo build -q -p spel 2>/dev/null)
SPEL_BIN="$SPEL_REPO/target/debug/spel"

echo "== Generating IDL from embedded sample =="
"$SPEL_BIN" generate-idl "$SAMPLE_SRC" 2>/dev/null > "$IDL"

run() {
    echo
    echo "── $* ──────────────────────────────"
    "$SPEL_BIN" --idl "$IDL" --program "$PROG_ID" --dry-run -- "$@" 2>&1
}

run initialize --caller "$CALLER"
run update-value --caller "$CALLER" --new-value 42
run poke --caller "$CALLER"

run admin-transfer --caller "$CALLER" --new-account "$NEW_ACCOUNT" --candidate Signer
run admin-renounce --caller "$CALLER"

echo
echo "Done."
