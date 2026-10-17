#!/bin/sh
# ledger.sh  fetches the cardano-ledger sources the V3 deposit cutoff rests on, at a pinned commit, and prints the deciding lines
SHA=${SHA:-178bd36839dcb57f257efc0e96615284d1632088}
get() { gh api "repos/IntersectMBO/cardano-ledger/contents/$1?ref=$SHA" -H 'Accept: application/vnd.github.raw'; }
echo "== Conway TxInfo.hs transTxCert (expect lines 586 to 595 gate the deposit on hardforkConwayBootstrapPhase)"
get eras/conway/impl/src/Cardano/Ledger/Conway/TxInfo.hs | /bin/cat -n | /usr/bin/sed -n '497,500p;518p;566,595p;649,653p'
echo "== Conway Era.hs (expect line 257: pvMajor pv == natVersion @9)"
get eras/conway/impl/src/Cardano/Ledger/Conway/Era.hs | /bin/cat -n | /usr/bin/sed -n '255,257p'
echo "== Era definitions (expect Babbage 7 to 8, Conway 9 to 11)"
get libs/cardano-ledger-core/internal/Cardano/Ledger/Internal/Definition/Era.hs | /bin/cat -n | /usr/bin/sed -n '121,133p'
echo "== Babbage max language (expect PlutusV2)"
get eras/babbage/impl/src/Cardano/Ledger/Babbage/Scripts.hs | /usr/bin/grep -n 'eraMaxLanguage'
