# randomx-sp1

`randomx-sp1` is the supported library interface for the repository's
SP1-optimized RandomX implementation. It accepts both the RandomX key and
hashing blob at runtime and returns the canonical 32-byte hash.

```rust
let digest: [u8; 32] = randomx_sp1::hash(&randomx_key, &hashing_blob);
```

The function constructs the complete 256 MiB Argon2d cache, derives light-mode
dataset items on demand, executes all eight RandomX programs, and implements
all four `CFROUND` modes in software on SP1. It does not embed a key or blob.
Each call constructs a fresh cache and is intentionally expensive. Consumers
must impose any application-specific input-length and resource limits.

Canonical compatibility is reviewed for RandomX's specified 0–60-byte key
domain. Longer-key behavior is implementation-defined. See the
[2026-09-05 review](../evidence/randomx-review-2026-09-05/README.md) for the
superscalar fix, the known large-key difference, and the production SP1
validation still required. The retained `v0.1.0` ELF predates the fix.

The `randomx-sp1-core`, `randomx-sp1-argon2`, and `randomx-softfp` crates are
implementation details. Their public Rust items support repository audit and
profiling tools but are not part of this crate's stable consumer API. The
default `randomx-sp1` feature set exposes only `hash`; audit features have no
compatibility guarantee.

This crate is GPL-3.0-only because it incorporates code derived from
RustDom-X. See the repository-level `ATTRIBUTION.md` for the complete lineage.
