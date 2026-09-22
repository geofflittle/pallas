#!/bin/sh
# repro-run.sh [branch...]: for each planted defect branch, puts its tree on the
# box, runs its test filter there, and prints the failed tests and the panic
# lines. With no arguments it runs every repro/eb-* branch.
S=/private/tmp/claude-501/-Users-geofflittle-code-sundae-v4/5201f29d-085c-478c-b226-c7d7811279ce/scratchpad/eb-fix
W=/Users/geofflittle/code/pallas-wt-eb
B=/opt/build/pallas-eb
if [ $# -eq 0 ]; then
  set -- $(/usr/bin/git -C "$W" for-each-ref --format='%(refname:short)' 'refs/heads/repro/eb-*')
fi
for br in "$@"; do
  label=repro-$(echo "$br" | tr '/' '-')
  echo "== $br $(/usr/bin/git -C "$W" rev-parse --short "$br")"
  sh "$S/sync.sh" "$br" || { echo "sync failed for $br"; continue; }
  if [ "$br" = repro/eb-indefinite-red ]; then
    ssh musashi-aws "$B/job.sh $label cargo test -p pallas-primitives --features unstable an_indefinite_body_is_refused" < /dev/null > /dev/null 2>&1 &
  else
    ssh musashi-aws "$B/job.sh $label cargo test -p pallas-traverse --features unstable leios" < /dev/null > /dev/null 2>&1 &
  fi
  sleep 5
  n=0
  until ssh musashi-aws "test -f $B/logs/$label.exit"; do
    n=$((n + 1))
    echo "  waiting on $label, $((n * 30))s, $(ssh musashi-aws "tail -c 120 $B/logs/$label.log | tr '\n' ' '")"
    sleep 30
  done
  echo "  exit $(ssh musashi-aws "cat $B/logs/$label.exit")"
  ssh musashi-aws "/usr/bin/grep -E '^test .* FAILED|^test result|panicked at|^  left|^ right|wrong refusal|assertion' $B/logs/$label.log | head -20"
done
echo "== all done"
