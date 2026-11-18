#!/bin/sh
# Does `rhizoid status` agree with graph-drift.sh, edge by edge, on the real graph? graph-drift.sh is the shell
# `rhizoid status` replaces (linux-phone-oco/docs research/base-graph/2026-11-18/graph-drift.sh). Both read the
# same clones at origin/main; this runs both and compares what each says about each edge:
#   - the edges either calls drift (FAIL; behind, or moved by hand with no item carrying it) are the same edges, the
#     same number of times, with the same numbers in what they say (a version, a count of lines or commits);
#   - every edge graph-drift.sh finds ok, rhizoid finds current or carried. (rhizoid says more: it also names the
#     tables and tool lists that are fine, where the shell is silent.)
#   agree-with-graph-drift.sh [PROJECTS [GRAPH_DRIFT]]
# PROJECTS is ~/Projects by default; GRAPH_DRIFT is read from docs' clone at origin/main when not given; RHIZOID names
# the program (rhizoid on PATH); FETCH=1 fetches every clone first, once, for both. Ends 0 when they agree, 1 when not.
set -eu
P=${1:-$HOME/Projects}
RZ=${RHIZOID:-rhizoid}
HERE=$(cd "$(dirname "$0")" && pwd)
GRAPH=${GRAPH:-$HERE/../examples/linux-phone-oco.graph.toml}
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
if [ -n "${2:-}" ]; then cp "$2" "$tmp/graph-drift.sh"
else git -C "$P/linux-phone-oco/docs" show origin/main:research/base-graph/2026-11-18/graph-drift.sh > "$tmp/graph-drift.sh"; fi

if [ "${FETCH:-0}" = 1 ]; then "$RZ" status --root "$P" --graph "$GRAPH" --fetch > /dev/null || true; fi
sh "$tmp/graph-drift.sh" "$P" > "$tmp/shell.txt" 2> "$tmp/shell.err" || [ $? = 1 ]
"$RZ" status --root "$P" --graph "$GRAPH" > "$tmp/rz.txt" || [ $? = 1 ]

# one line per finding: FOLLOWER CHECK ok|FAIL NUMBERS (the digits each says, in order)
norm_shell() {
	awk '{
		v = $1; f = $2; c = $3; sub(/:$/, "", c)
		if (f !~ /\//) f = (f == "lab-machine") ? "grenudi/lab-machine" : "linux-phone-oco/" f
		if (c == "tad-pin") c = ($0 ~ /does not list it/) ? "tool-listed" : "tool-pin"
		n = $0; sub(/^[^:]*: /, "", n); gsub(/[^0-9]+/, " ", n)
		print f, c, v, n
	}' "$1"
}
norm_rz() {
	awk '{
		v = ($0 ~ /\(on its way up: / || $1 == "current") ? "ok" : "FAIL"
		f = $2; c = $5; sub(/:$/, "", c)
		n = $0; sub(/ \(on its way up:.*$/, "", n)
		sub(/^[^ ]+ +[^ ]+ <- [^ ]+ +[^ ]+: /, "", n); gsub(/[^0-9]+/, " ", n)
		print f, c, v, n
	}' "$1" | grep -v ' checks: '
}
grep -E '^(ok|FAIL) ' "$tmp/shell.txt" | norm_shell /dev/stdin | sort > "$tmp/shell.n"
grep -E '^(current|behind|moved-by-hand) ' "$tmp/rz.txt" | norm_rz /dev/stdin | sort > "$tmp/rz.n"

grep ' FAIL ' "$tmp/shell.n" > "$tmp/shell.fail" || true
grep ' FAIL ' "$tmp/rz.n" > "$tmp/rz.fail" || true
# an ok the shell prints must be an ok in rhizoid: compare the (follower, check) of each
awk '$3 == "ok" { print $1, $2 }' "$tmp/shell.n" | sort > "$tmp/shell.ok"
awk '$3 == "ok" { print $1, $2 }' "$tmp/rz.n" | sort -u > "$tmp/rz.ok"
comm -23 "$tmp/shell.ok" "$tmp/rz.ok" > "$tmp/ok.missing"

echo "graph-drift.sh: $(grep -c ' ok ' "$tmp/shell.n" || true) ok, $(wc -l < "$tmp/shell.fail") FAIL"
echo "rhizoid status: $(grep -c ' ok ' "$tmp/rz.n" || true) ok (current or carried), $(wc -l < "$tmp/rz.fail") drift"
echo "by edge kind (FAIL in graph-drift.sh / drift in rhizoid status):"
for c in slice-pin own-behind own-part layer-pin layer-owner layer-follow tool-pin tool-listed fork-track; do
	a=$(awk -v c="$c" '$2 == c' "$tmp/shell.fail" | wc -l); b=$(awk -v c="$c" '$2 == c' "$tmp/rz.fail" | wc -l)
	printf '  %-13s %3d / %3d\n' "$c" "$a" "$b"
done
bad=0
if ! diff "$tmp/shell.fail" "$tmp/rz.fail" > "$tmp/fail.diff"; then bad=1; echo "the drift differs (< graph-drift.sh, > rhizoid):"; cat "$tmp/fail.diff"; fi
if [ -s "$tmp/ok.missing" ]; then bad=1; echo "ok in graph-drift.sh, not current in rhizoid:"; cat "$tmp/ok.missing"; fi
[ $bad = 0 ] && echo "agree"
exit $bad
