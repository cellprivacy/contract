# Fuzz targets

Two libFuzzer targets: `smt` drives the tree directly, `release_funds` drives the
entrypoint that moves money with the operator's arguments under the fuzzer's
control.

```sh
cargo +nightly fuzz run smt
cargo +nightly fuzz run release_funds
```

## Known limitation

These do not link on macOS ARM. Soroban contracts must declare
`crate-type = ["lib", "cdylib"]`, cargo-fuzz builds every crate type, and
linking the instrumented `cdylib` fails:

```
ld: initializer pointer has no target in ... escrow.rcgu.o
```

Disabling the sanitizer does not help; the failure is the sancov-instrumented
dylib itself. Verified against nightly with and without `-s none`.

The same properties run in plain `cargo test` through `src/test/props.rs`, which
is where they belong for CI anyway. These targets exist for interactive
campaigns on a platform where they link; that has not been confirmed here, so
treat them as unrun rather than as passing.
