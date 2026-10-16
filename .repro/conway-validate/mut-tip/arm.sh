#!/bin/sh
# arm.sh ARM [features]  runs the pallas-validate suite in $A/w-mut with MUT=ARM, prints failing tests and their panic lines.
# ARM may be a comma list. ARM=none is the baseline. REACH=1 arm.sh none lists the reach points the suite hits.
A=${A:-/opt/build/cvfix-1015}
ARM=$1; FEAT=${2:-phase2}
L=$A/logs/mut-$(echo $ARM | tr , _)-$FEAT${REACH:+-reach}.log
mkdir -p $A/logs
docker run --rm --name "cvfix-mut" -e MUT=$ARM ${REACH:+-e REACH=1} -v "$A/w-mut:/w" -v "$A/target-w-mut:/target" -v /opt/build/shared-cargo:/cargo -e CARGO_HOME=/cargo -e PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_TEST_DEBUG=0 -e CARGO_TERM_COLOR=never -e RUST_BACKTRACE=short -w /w pallas-ci:1 cargo test -p pallas-validate --features $FEAT --no-fail-fast -- --nocapture --test-threads=1 > $L 2>&1
C=$?
echo "arm $ARM features=$FEAT exit=$C active=$(/bin/grep -c MUTANT_ACTIVE $L) compiled=$(/bin/grep -cE '^ +Compiling ' $L) results=$(/bin/grep -c '^test result' $L)"
/bin/grep -E '^error' $L | head -20
/bin/grep -E '^test result: FAILED' $L
/bin/grep -E '^test .* FAILED$' $L | sort -u
if [ -n "$REACH" ]; then /bin/grep -oE 'REACHED [A-Za-z_0-9]+' $L | sort | uniq -c; fi
