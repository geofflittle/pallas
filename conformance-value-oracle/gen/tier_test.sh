#!/bin/sh
# Checks that tier.sh runs its pairs four at a time, fails when a pair fails, that a tier generates the arb values of each kind once, and that vrun.sh marks a pair compared only when each compare wrote its counts, with a docker that logs each generator call.
# Usage: tier_test.sh <empty work dir>
set -u
W=$(cd "$1" && pwd)
G=$(cd "$(dirname "$0")" && pwd)
failed=0

expect() {
  if [ "$2" = "$3" ]; then
    echo "TIER-TEST $1 PASS"
  else
    echo "TIER-TEST $1 FAIL got=\"$2\" want=\"$3\""
    failed=1
  fi
}

# A vrun that marks its pair started and running, waits up to five seconds for four pairs to have started, then writes to its log whether four had and whether at most four ran half a second later, and exits $FAIL_KIND's status.
cat >"$W/vrun-stub.sh" <<'EOF'
P=$OUT/marks
mkdir -p "$P/started" "$P/running"
touch "$P/started/$3.$4" "$P/running/$3.$4"
t=0
while [ "$(ls "$P/started" | wc -l)" -lt 4 ] && [ "$t" -lt 50 ]; do
  sleep 0.1
  t=$((t + 1))
done
barrier=met
if [ "$(ls "$P/started" | wc -l)" -lt 4 ]; then barrier=unmet; fi
sleep 0.5
bound=ok
if [ "$(ls "$P/running" | wc -l)" -gt 4 ]; then bound=over; fi
rm "$P/running/$3.$4"
echo "$3:$4:$barrier:$bound"
[ "$3" != "${FAIL_KIND:-}" ]
EOF

# The exit and the log line of each pair of a tier over eight pairs with the stub vrun, failing the pairs of the kind $2.
pooled() {
  O=$W/pool-$1
  rm -rf "$O"
  st=0
  PAIRS="tx:arb tx:cddl body:arb body:cddl wits:arb wits:cddl ppu:arb ppu:cddl" VRUN=$W/vrun-stub.sh TALLY=true FAIL_KIND=$2 \
    sh "$G/tier.sh" "$O" 1 1 >/dev/null 2>&1 || st=$?
  echo "exit=$st $(for p in tx.arb tx.cddl body.arb body.cddl wits.arb wits.cddl ppu.arb ppu.cddl; do cat "$O/$p.log"; done | paste -sd ' ' -)"
}
# Must run four pairs at once and no more, must pass when every pair passes, and must fail when one pair fails.
expect pool_four "$(pooled pass '')" "exit=0 tx:arb:met:ok tx:cddl:met:ok body:arb:met:ok body:cddl:met:ok wits:arb:met:ok wits:cddl:met:ok ppu:arb:met:ok ppu:cddl:met:ok"
expect pool_fails "$(pooled fail wits | cut -d' ' -f1)" "exit=1"

# A docker that logs each generator call and prints a value per asked value, a ledger result per input line.
mkdir -p "$W/bin"
cat >"$W/bin/docker" <<EOF
#!/bin/sh
while [ "\$1" != "$(sh "$G/image-name.sh" conformance-gen-hs)" ]; do shift; done
shift 2
echo "\$*" >>"$W/docker.log"
case \$1 in
  gen)
    i=0
    while [ "\$i" -lt "\$4" ]; do echo "{\"i\": \$i, \"hex\": \"80\"}"; i=\$((i + 1)); done
    ;;
  oracle) while read -r line; do echo '{"ok": false}'; done ;;
esac
EOF
chmod +x "$W/bin/docker"
printf '#!/bin/sh\ncat >/dev/null\n' >"$W/probe"
chmod +x "$W/probe"

# The generator calls for arb values of each kind of a tier over three tx pairs drawn from arb values and one body pair.
generated() {
  O=$W/arb
  rm -rf "$O" "$W/docker.log"
  PATH=$W/bin:$PATH PROBE=$W/probe PAIRS="tx:arb tx:mutate tx:empty14 body:mutate" TALLY=true \
    sh "$G/tier.sh" "$O" 3 1 >/dev/null 2>&1
  echo "tx=$(grep -c -x 'gen tx arb 3 1' "$W/docker.log") body=$(grep -c -x 'gen body arb 3 1' "$W/docker.log")"
}
# Must generate the arb values of a kind once whether or not its arb pair runs.
expect arb_once "$(generated)" "tx=1 body=1"

# The compared mark of an eb arb pair of three values whose probe answers each value it reads up to $1 values and then stops.
compared() {
  O=$W/compared-$1
  rm -rf "$O"
  printf '#!/bin/sh\nhead -n %s | while read -r line; do echo "{\\"ok\\": true}"; done\n' "$1" >"$W/probe-$1"
  chmod +x "$W/probe-$1"
  PATH=$W/bin:$PATH PROBE=$W/probe-$1 OUT=$O sh "$G/vrun.sh" 3 1 eb arb >/dev/null 2>&1
  cat "$O/eb.arb.compared" 2>/dev/null || echo unmarked
}
# Must not fire: a probe that stops before the last value leaves the compare unfinished.
expect compared_cut "$(compared 1)" unmarked
# Must mark a compare that read a result of the probe and the ledger for every value.
expect compared_whole "$(compared 3)" 3
# Must not fire: a compare that wrote no counts is unfinished, whatever the other compares wrote.
rm -rf "$W/py"
cp -R "$G" "$W/py"
printf 'import sys\nsys.exit(1)\n' >"$W/py/schemacmp.py"
O=$W/compared-schema
PATH=$W/bin:$PATH PROBE=$W/probe-3 OUT=$O sh "$W/py/vrun.sh" 3 1 tx arb >/dev/null 2>&1
expect compared_schema_crash "$(cat "$O/tx.arb.compared" 2>/dev/null || echo unmarked) value_counts=$(ls "$O/tx.arb.counts.json" 2>/dev/null | wc -l | tr -dc 0-9)" "unmarked value_counts=1"

if [ "$failed" -eq 0 ]; then
  echo "TIER-TEST RESULT PASS"
else
  echo "TIER-TEST RESULT FAIL"
  exit 1
fi
