# randomx-softfp

Internal exact binary64 arithmetic for the RandomX floating-point domain and
all four consensus rounding modes. It is used by `randomx-sp1` on SP1's RV64IM
target and is not a separately supported consumer API.

This component is available under MIT OR Apache-2.0, as declared in its
`Cargo.toml`. See the repository-level `ATTRIBUTION.md` for project context.

## SP1 validation guest

The package also owns an opt-in SP1 guest that checks fixed arithmetic vectors,
benchmarks every operation in all four rounding modes, and emits cycle-tracker
regions. From the repository root, build it with:

```bash
cargo prove build --locked -p randomx-softfp \
  --binaries randomx-softfp-guest --features sp1-guest \
  --elf-name randomx-softfp-guest --output-directory target/softfp-guest
```

Run the resulting ELF with `randomx-sp1-executor --profile`. Its expected
32-byte public value is:

```text
4cdd7978f088db5e736f667466702d616c6c2d766563746f72732d7061737321
```
