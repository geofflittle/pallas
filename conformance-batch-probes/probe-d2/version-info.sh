#!/bin/sh
# Feeds the generated block, its two 2^32 mutations and its indefinite form to the ledger oracle.
# usage: sh version-info.sh PALLAS_TREE   (relative to /opt/build/pallas-conformance-1024)
# Expected:
#   True BlockHeaderVersionInfo {bhviHighestSupportedMajorVersion = 1004951745, bhviSelfReportedSoftwareTag = 3942894501}
#   False DecoderErrorDeserialiseFailure "oracle" (DeserialiseFailure 398 "expected word32")
#   False DecoderErrorDeserialiseFailure "oracle" (DeserialiseFailure 403 "expected word32")
#   True BlockHeaderVersionInfo {bhviHighestSupportedMajorVersion = 1004951745, bhviSelfReportedSoftwareTag = 3942894501}
cd /opt/build/pallas-conformance-1024
B=$(cat "$1/test_data/dijkstra-generated/block-header-version-info.hex")
I=821a3be658c11aeb03cba5
{
  echo "$B"
  echo "$B" | sed "s/$I/821b00000001000000001aeb03cba5/"
  echo "$B" | sed "s/$I/821a3be658c11b0000000100000000/"
  echo "$B" | sed "s/$I/9f1a3be658c11aeb03cba5ff/"
} | docker run --rm -i --user "$(id -u):$(id -g)" -v /opt/build/conformance-gen-1024:/work conformance-gen-hs:2 /work/bin/conformance-gen oracle block \
  | python3 -I -c 'import sys, json, re
for line in sys.stdin:
    r = json.loads(line)
    info = re.findall(r"BlockHeaderVersionInfo \{[^}]*\}", r.get("show", ""))
    print(r["ok"], info[0] if info else r.get("err"))'
