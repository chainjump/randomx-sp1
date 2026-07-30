# randomx-sp1 Argon2 internals

This internal crate is derived from `rust-argon2` 2.1.0 and retains only the
fixed Argon2d v1.3 operation required by RandomX: one lane, three iterations,
262,144 one-kibibyte blocks, and the salt `RandomX\x03`. Callers supply only
the RandomX key through `initialize_randomx`; the resulting block allocation
is consumed by `randomx-sp1-core`.

It is deliberately not a password-hashing library and exposes no generic
configuration, encoded-hash, verification, Argon2i, or Argon2id API. It is not
published or supported as an independent consumer API. The unmodified generic
implementation is retained only as a dev-dependency so the test suite can
compare complete 256 MiB caches for multiple key shapes and frozen digests.

The operation allocates 256 MiB and does not clear that allocation before it
is released. RandomX keys are normally public epoch data, not passwords.
Full lineage is recorded in the repository's `ATTRIBUTION.md`.


## License

This fork is dual licensed under the [MIT](LICENSE-MIT) and
[Apache 2.0](LICENSE-APACHE) licenses, the same licenses as the Rust compiler.


## Contributions

Contributions are accepted under the existing dual-license terms.
