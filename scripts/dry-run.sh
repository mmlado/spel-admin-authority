#!/usr/bin/env bash
# Dry-run the admin-authority sample's four instructions via the SPEL CLI.
#
# Fully offline: dry-run resolves accounts, PDAs, and instruction data and
# prints the transaction without submitting. No node, no built guest binary
# (a placeholder program id is enough).
#
# Usage:  scripts/dry-run.sh [path-to-spel-repo]
# Output: prints to stdout; CI or docs can redirect to a file.
#
# Enum arguments (new_admin: AdminCandidate) use the CLI's defined-type
# syntax: a bare variant name for unit variants (Signer), or a one-key
# JSON object for payload variants ({"Pda": {"program_id": "..", "seed": ".."}}).

set -uo pipefail

SPEL_REPO="${1:-$(dirname "$0")/../../spel}"
SAMPLE_SRC="$(dirname "$0")/../admin-authority-sample/src/main.rs"
PROG_ID="$(printf 'ab%.0s' {1..32})"          # placeholder, fine for dry-run
CALLER="$(printf '11%.0s' {1..32})"
NEW_ADMIN="$(printf '22%.0s' {1..32})"
IDL="$(mktemp --suffix .idl.json)"
trap 'rm -f "$IDL"' EXIT

echo "== Building spel CLI =="
(cd "$SPEL_REPO" && RISC0_SKIP_BUILD=1 cargo build -q -p spel 2>/dev/null)
SPEL_BIN="$SPEL_REPO/target/debug/spel"

echo "== Generating IDL from sample =="
"$SPEL_BIN" generate-idl "$SAMPLE_SRC" 2>/dev/null > "$IDL"

run() {
    echo
    echo "── $* ──────────────────────────────"
    "$SPEL_BIN" --idl "$IDL" --program "$PROG_ID" --dry-run -- "$@" 2>&1
}

run update-value --caller "$CALLER" --new-value 42
run admin-renounce --caller "$CALLER"

run admin-initialize --caller "$CALLER"

# NOTE: a Signer-candidate transfer is only valid on-chain when the NEW admin
# also signs (is_authorized comes from the tx witness set). The CLI's
# witness-exchange feature (partial-tx blob, spel sign / spel submit) is
# planned separately; until it lands this dry-run shows the shape only.
run admin-transfer --caller "$CALLER" --new-admin-account "$NEW_ADMIN" --new-admin Signer

# Payload variant: transfer admin to a PDA, passing the enum as JSON.
PDA_PROGRAM="$(printf 'cd%.0s' {1..32})"
PDA_SEED="$(printf 'ef%.0s' {1..32})"
run admin-transfer --caller "$CALLER" --new-admin-account "$NEW_ADMIN" \
    --new-admin "{\"Pda\": {\"program_id\": \"$PDA_PROGRAM\", \"seed\": \"$PDA_SEED\"}}"

echo
echo "Done."
