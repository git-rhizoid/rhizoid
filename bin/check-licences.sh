#!/bin/sh
# check-licences: is every tracked file covered by a licence - an SPDX-License-Identifier line in its first 20 lines,
# or an annotation of REUSE.toml naming its path - and does every licence named have its text in the repository?
# The REUSE way (reuse.software), as a shell check because `reuse lint` is Python (decision 2026-11-19-licences-for-
# our-repositories). Reads git's list of tracked files; changes nothing.
#   bin/check-licences.sh        ends 0 when every file is covered, 1 and lists the ones that are not
# A path in REUSE.toml is a glob as REUSE reads it: `*` is within one folder, `**` crosses folders, `?` is one character.
# The licence texts and REUSE.toml itself need no annotation. A licence's text is in LICENSE-MIT, LICENSE-APACHE or
# LICENSE-CC-BY-SA-4.0.
set -eu
cd "$(git rev-parse --show-toplevel)"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0

# the paths and licences of every annotation, one pattern per line, as an extended regular expression
[ -f REUSE.toml ] || { echo "check-licences: no REUSE.toml"; fail=1; : > "$tmp/globs"; }
if [ -f REUSE.toml ]; then
	awk '
		/^\[\[annotations\]\]/ { on = 0 }
		/^path[ \t]*=/ { on = 1; multi = ($0 ~ /\[/ && $0 !~ /\]/) }
		on { s = $0; while (match(s, /"[^"]*"/)) { print substr(s, RSTART + 1, RLENGTH - 2); s = substr(s, RSTART + RLENGTH) } }
		on && (!multi || /\]/) { on = 0 }
	' REUSE.toml |
		sed -e 's/[.+^$(){}|]/\\&/g' -e 's|\*\*/|\x01|g' -e 's|\*\*|\x02|g' -e 's|\*|[^/]*|g' -e 's|?|[^/]|g' \
			-e 's|\x01|(.*/)?|g' -e 's|\x02|.*|g' > "$tmp/globs"
	sed -n 's/^SPDX-License-Identifier[ \t]*=[ \t]*"\(.*\)"/\1/p' REUSE.toml | sed 's/ \(AND\|OR\|WITH\) /\n/g' | sort -u > "$tmp/licences"
	while read -r id; do
		case $id in
		MIT) text=LICENSE-MIT ;;
		Apache-2.0) text=LICENSE-APACHE ;;
		CC-BY-SA-4.0) text=LICENSE-CC-BY-SA-4.0 ;;
		*) echo "check-licences: REUSE.toml names $id, whose text this check does not know"; fail=1; continue ;;
		esac
		[ -s "$text" ] || { echo "check-licences: REUSE.toml names $id and $text is missing or empty"; fail=1; }
	done < "$tmp/licences"
fi

git ls-files | while IFS= read -r f; do
	case $f in LICENSE-* | REUSE.toml) continue ;; esac
	[ -f "$f" ] || continue
	if head -n 20 "$f" | grep -aq 'SPDX-License-Identifier:'; then continue; fi
	if [ -s "$tmp/globs" ] && printf '%s\n' "$f" | grep -Eqxf "$tmp/globs"; then continue; fi
	echo "check-licences: $f has no SPDX-License-Identifier and no REUSE.toml annotation covers it"
done > "$tmp/uncovered"
cat "$tmp/uncovered"
[ -s "$tmp/uncovered" ] && fail=1
[ $fail = 0 ] && echo "check-licences: every tracked file is covered"
exit $fail
