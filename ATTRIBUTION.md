# Attribution

## RandomX implementation lineage

The internal `randomx-sp1-core` crate is derived from
[RustDom-X](https://github.com/snap-coin/rustdom-x) 1.1.0 by the Snapchain
project. RustDom-X describes itself as derived primarily from
[mithril](https://github.com/Ragnaroek/mithril), with architecture-specific
operations removed for portability.

The local implementation has since received substantial SP1-specific cache,
dataset, interpreter, floating-point, correctness, and performance work. The
new package names identify this maintained implementation without obscuring
its origin.

The derived core and public `randomx-sp1` library remain available under
GPL-3.0-only. The original GPL text is retained at
`randomx-core/LICENSE`.

## Argon2 implementation lineage

The internal `randomx-sp1-argon2` crate is derived from Martijn Rijkeboer's
`rust-argon2` 2.1.0 implementation and retains its MIT/Apache-2.0 licensing
and source notices. It contains a specialized RandomX Argon2d cache path in
addition to the inherited generic API and tests.

Renaming these internal packages does not remove or replace any copyright,
license, or authorship notice in their source files.
