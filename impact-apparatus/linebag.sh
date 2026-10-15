# Print the multiset difference of lines, leading whitespace stripped, between two commits over pallas-utxorpc and test_data.
# Usage: bash linebag.sh <repo> <from> <to>
export LC_ALL=C
repo=$1
bag() {
  /usr/bin/git -C "$repo" ls-tree -r --name-only "$1" -- pallas-utxorpc test_data |
    while read -r f; do /usr/bin/git -C "$repo" show "$1:$f"; done |
    sed 's/^[[:space:]]*//' | sort
}
comm -3 <(bag "$2") <(bag "$3")
