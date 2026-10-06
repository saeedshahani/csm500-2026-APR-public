#!/usr/bin/env bash
# The dependency checkout is external to the source tree; no research copy is used.
set -euo pipefail
: "${NUMERICAL_MATHLIB:?Set NUMERICAL_MATHLIB to the pinned Mathlib checkout}"
: "${NUMERICAL_LEAN_BIN:?Set NUMERICAL_LEAN_BIN to the Lean 4.19.0 bin directory}"
expected=c44e0c8ee63ca166450922a373c7409c5d26b00b
test "$(git -C "$NUMERICAL_MATHLIB" rev-parse HEAD)" = "$expected"
test "$(git -C "$NUMERICAL_MATHLIB" status --porcelain --untracked-files=no)" = ""
"$NUMERICAL_LEAN_BIN/lean" --version | grep -F 'version 4.19.0'
proof_root=$(cd "$(dirname "$0")/.." && pwd)
export PATH="$NUMERICAL_LEAN_BIN:$PATH"
"$NUMERICAL_LEAN_BIN/lake" --dir "$NUMERICAL_MATHLIB" env lean "$proof_root/proofs/numerical/LocalCycles.lean"
