# Running the consumer compatibility probe

The probe is a crate outside the pallas workspace that depends on
pallas-utxorpc, pallas-traverse, pallas-primitives, pallas-codec and
pallas-validate by path, with no feature enabled. It stands in for a caller
that took pallas 1.4.0 from crates.io on a caret range.

Place this directory at the root of a pallas worktree, beside
`pallas-utxorpc`, and run the commands below from that root.

## Arm 1, the released surface still compiles

    cargo build --manifest-path compat-probe/Cargo.toml

Expected verdict against a release that keeps faith with 1.4.0:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.36s

Expected verdict against `dcdf784` (the u5c mapping before the rework), 16
errors covering nine distinct defects:

    src/main.rs:41:17: error[E0308]: mismatched types      map_purpose re-signed
    src/main.rs:63:17: error[E0308]: mismatched types      map_native_script re-signed
    src/main.rs:65:20: error[E0599]: no associated function map_any_script
    src/main.rs:105:41: error[E0599]: no associated function map_alonzo_compatible_cert
    src/main.rs:112:41: error[E0599]: no associated function map_conway_cert
    src/main.rs:122:20: error[E0599]: no associated function map_conway_pparams_update
    src/main.rs:138:20: error[E0599]: no associated function map_conway_gov_action
    src/main.rs:172:11: error[E0004]: non-exhaustive patterns, MultiEraHeader
    src/main.rs:183:11: error[E0004]: non-exhaustive patterns, Error::UnknownHeaderEnvelopeTag

## Arm 2, the restored items really carry a deprecation

Arm 1 allows the deprecated lint, so it would pass just as well if the
restored items carried no deprecation at all. This arm denies the lint and
MUST fail.

    cargo build --manifest-path compat-probe/Cargo.toml --example deprecation

Expected verdict, seven errors, one per restored item:

    error: use of deprecated method ... map_purpose
    error: use of deprecated associated function ... map_native_script
    error: use of deprecated method ... map_any_script
    error: use of deprecated method ... map_alonzo_compatible_cert
    error: use of deprecated method ... map_conway_cert
    error: use of deprecated method ... map_conway_pparams_update
    error: use of deprecated method ... map_conway_gov_action
    error: could not compile `pallas-compat-probe` (example "deprecation") due to 7 previous errors

A build that succeeds means a restored item lost its deprecation.

## Arm 3, the leios enums need no marking

This arm belongs to the endorser block branch, where `pallas-traverse`
carries `src/leios.rs`. It asks whether `leios::Error` and
`leios::PendingAnnouncement` should be marked `#[non_exhaustive]` before
release.

    cargo run --manifest-path compat-probe/Cargo.toml --example leios_exhaustive

Verdict measured on 2026-09-20 against the merged endorser head `f555c7b`:

    error[E0432]: unresolved import, could not find `leios` in `pallas_traverse`
    note: found an item that was configured out
      --> /pallas/pallas-traverse/src/lib.rs:117:9
    116 | #[cfg(feature = "unstable")]
        |       -------------------- the item is gated behind the `unstable` feature

The module is already behind the feature, so no caller on the stable surface
can match either enum and the marking is unnecessary.
