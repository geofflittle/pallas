#!/bin/sh
# gen.sh REV  writes one patch per arm of arms.tsv into arms/, each against commit REV of the pallas repository
# arms.tsv rows: name, file, line, exact old line, new line (DELETE removes it); rows of one arm and file run in listed order, so list later lines first
D=$(cd "$(dirname "$0")" && pwd)
R=${R:-/Users/geofflittle/code/pallas}
T=$(mktemp -d)
/usr/bin/git -C "$R" archive "$1" pallas-validate/src | tar -x -C "$T" || exit 2
rm -f "$D"/arms/*.patch
for N in $(cut -f1 "$D/arms.tsv" | uniq); do
  rm -rf "$T/b"; mkdir -p "$T/b"; cp -R "$T/pallas-validate" "$T/b/"
  awk -F '\t' -v n="$N" '$1 == n' "$D/arms.tsv" | while IFS='	' read -r name file line old new; do
    awk -v ln="$line" -v old="$old" -v new="$new" 'NR == ln { if ($0 != old) { print "MISMATCH " FILENAME ":" ln > "/dev/stderr"; exit 3 } if (new != "DELETE") print new; next } { print }' "$T/b/$file" > "$T/tmp" || { echo "arm $N row $line FAILED"; exit 3; }
    mv "$T/tmp" "$T/b/$file"
  done || exit 3
  (cd "$T" && mkdir -p a && rm -rf a/pallas-validate && cp -R pallas-validate a/ && diff -ruN a b | /usr/bin/grep -v '^diff -ruN') > "$D/arms/$N.patch"
  echo "$N hunks=$(/usr/bin/grep -c '^@@' "$D/arms/$N.patch")"
done
rm -rf "$T"
