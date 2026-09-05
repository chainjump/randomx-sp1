# Review of RandomX canonical compatibility, 2026-09-05

Archived review record: the working-tree fix described below is now committed
as `48e096823fd332076c2b5ab0e272beee27b2b473`. Source findings and observed results
are unchanged. See [release status](README.md) and [probe-source notes](probe-sources.md).

Reviewed repository: `/root/experiment/randomx-sp1`, HEAD
`a7de1d52c5d2c507f7c8bd6cbaa95f3fa52a8b0a`, including the existing uncommitted
high-multiply superscalar metadata fix. No implementation changes were made in
this review pass. Per-file source digests are in [reviewed-source-sha256.json](canonical/reviewed-source-sha256.json).

Oracle: upstream RandomX v1.2.3, commit
[`12f2c2ffe2108d6cf54c391fee33c8bc3646cdab`](https://github.com/tevador/RandomX/tree/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab),
downloaded as a clean source archive from GitHub and built as a C++ shared library
in the original temporary workspace. Runtime uses the canonical interpreter and light-mode cache.
All probes ran on the host. The SP1 prover was not compiled. The guest ELF was
not rebuilt or executed. The demonstration EVM prover was excluded.

No additional critical hash discrepancy was found for the specified RandomX
key domain (0–60 bytes). Two noncritical behavioral differences were reproduced.
Two additional source metadata differences are inert under the current
superscalar scheduling rules.

1. Key lengths at or above 2^32 bytes differ from the C++ API implementation.

At [argon2/src/core.rs:349](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/argon2/src/core.rs#L349),
Rust serializes the key length as a u32 but then hashes the entire key slice.
Canonical [dataset.cpp:79](https://github.com/tevador/RandomX/blob/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab/src/dataset.cpp) assigns `(uint32_t)keySize`
to the Argon2 password length; canonical [argon2_core.c:351](https://github.com/tevador/RandomX/blob/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab/src/argon2_core.c)
uses that truncated length for both the length field and the amount of password
data consumed. Consequently the Argon2 initialization inputs differ starting
at 4,294,967,296 bytes.

Reproduction: key = exactly 4,294,967,296 zero bytes; blob = ASCII
`canonical large-key boundary` (no newline).

| Result | Canonical C++ | Rust |
| --- | --- | --- |
| Cache BLAKE2b-256 | `faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15` | `9041c5b980d9b5a4e97e9d4ce9379aae7a13fdab7515b36d0ab60ff9de2fe6fb` |
| RandomX hash | `2d6b69641e8607c5dca4b5f295fac63d2996434f5c722509c3583eeb2a777b68` | `4a60a19422a2a94dfa77780d6a6049691be653abf0a62dc293e7317a02068e68` |

The canonical result also matched its result for an empty key. An anonymous,
read-only zero-filled mapping supplied the large input, avoiding a 4 GiB
physical allocation. The C++ probe calls `randomx::initCache` directly, which
is the same default initializer used by the C API, bypassing only the C API's
large `std::string` key memoization copies. Hash calculation uses the public
`randomx_calculate_hash` API. Rust uses the repository's real cache and
`hash_with_vm_for_audit`, which calls the same implementation as public `hash`.

This does not violate the RandomX consensus specification: its key limit is
60 bytes, and it expressly calls longer-key behavior implementation-defined
([specification, section 7](https://github.com/tevador/RandomX/blob/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab/doc/specs.md#7-dataset)).
It does limit the README's unrestricted key-length/canonical-compatibility
claim. Ordinary 32-byte Monero keys are unaffected. A documented and enforced
key-length policy would resolve the ambiguity; matching the C++ extension
beyond 4 GiB would instead require matching its password-length truncation.

2. The AES hash helper rejects valid small inputs when debug assertions are enabled.

[hash.rs:15](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/randomx-core/src/hash.rs#L15) requires
`input.len()` to be a multiple of 64, but `input` contains u64 words. A complete
64-byte block is eight words. The unconditional check immediately below it
correctly tests a multiple of eight.

Calling `hash_aes_1rx4(&[0u64; 8])` panics in a build with debug assertions.
The release implementation and canonical AES hash both accept and agree on
that input. This does not affect complete RandomX hashes: their 2 MiB
scratchpads satisfy both checks. Reproducer: [debug-aes.rs](probe-sources.tar.gz);
observed expected panic: [debug-aes.log](canonical/debug-aes.txt).

3. Superscalar metadata differs without changing the current algorithm.

| Location | Rust | Canonical | Why it is inert |
| --- | --- | --- | --- |
| [IROR_C construction:286](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/randomx-core/src/superscalar.rs#L286) | `group_par_is_source = true`, `op_group_par = 0` | `false`, `-1` | IROR_C has no source-selection macro-op, so the flag is never used. Every instruction in this operation group receives the same constant in each implementation, preserving all equality comparisons in destination selection. |
| [MOP_MOV_RR:458](https://github.com/chainjump/randomx-sp1/blob/48e096823fd332076c2b5ab0e272beee27b2b473/randomx-core/src/superscalar.rs#L458) | latency 1 | latency 0 | These moves are eliminated, are not the result-writing macro-op, and their dependency cycle is overwritten before any dependent operation consumes it. The high-multiply result comes from the multiply macro-op. |

Static tracing of the generator supports these conclusions. In addition, all
32,768 sampled serialized programs and their published integer scheduling
metrics matched. These metadata deviations are distinct from the already
fixed high-multiply random-group-parameter bug.

Host differential checks completed successfully:

| Area | Coverage | Log |
| --- | --- | --- |
| Blake2Generator | 2,097,152 mixed byte/u32 requests over 4,096 seed/nonce cases, including seeds on both sides of 60 bytes | [generator.log](canonical/generator.txt) |
| Superscalar generator and ordinary executor | 32,768 serialized programs; size, address register, code size, macro-op count, decode cycles, CPU/ASIC latency and per-register latencies, multiplication count, and execution result | [generator.log](canonical/generator.txt) |
| Production unchecked superscalar executor | Another 32,768 programs with integer boundary and random register values | [superscalar-exec.log](canonical/superscalar-exec.txt) |
| AES stages | 128 cases comparing 1R fill and final state, 4R program generation, and AES hash; includes complete 2 MiB scratchpads | [generator.log](canonical/generator.txt) |
| Portable AES rounds used on SP1 | 216,384 encryption/decryption round comparisons; every byte value in every state position and random state/key pairs | [portable-aes.log](canonical/portable-aes.txt) |
| Optimized VM decoder and effects | 2,359,296 direct C++ comparisons: every opcode and modifier, register aliases, boundary immediates, integer/float edge values, all four rounding modes, forced taken branches, scratchpad writes, and 1,024 mixed programs for register-usage/branch-target tracking | [vm.log](canonical/vm.txt) |
| Complete Argon2 caches | 16 complete 256 MiB cache digests, with key lengths 0, 1, 31, 32, 59, 60, 61, 64, 65, 127, 128, 129, 257 and 4,096 bytes (32 and 60 repeated with fresh keys) | [fullhash.log](canonical/fullhash.txt) |
| Dataset derivation | 2,048 items, including cache/base/extra-dataset boundaries and the final valid item | [fullhash.log](canonical/fullhash.txt) |
| Complete hashes | 32 fresh key/blob pairs with empty and nonempty inputs; blob lengths through 4,096 bytes | [fullhash.log](canonical/fullhash.txt) |
| Large-key boundary | Additional confirmed cache/hash discrepancy for exactly 2^32 zero bytes | [fullhash.log](canonical/fullhash.txt) |

The VM probe copies the production library source unchanged and appends a
wrapper exposing its private decoder/effects. The portable AES probe does the
same for the private software AES routines. Source-prefix identity was checked
against the working tree. Full hashes and cache/dataset checks depend directly
on the repository crates. No alternate Rust implementation serves as the oracle.
The C++ shims adapt layouts, preserve host rounding state, and satisfy canonical
AES buffer-alignment requirements.

These are finite differential checks. They do not establish equivalence for all
possible keys, blobs, or execution states, nor validate code generation for a
new SP1 ELF. The earlier arithmetic-only Berkeley SoftFloat comparison remains
separate evidence; it was not rerun or counted in the table above.

Portability observation: Argon2 initialization writes little-endian hash bytes
directly into the native u64 storage in `Block::as_u8_mut`. This agrees with SP1
and the reviewed host; a future big-endian port needs explicit byte-order
conversion. No big-endian target was executed in this review.

Reproduction files are [shim.cpp](probe-sources.tar.gz), [aes-shim.cpp](probe-sources.tar.gz), and
[the standalone Rust probe package](probe-sources.tar.gz). Build the canonical
`randomx` CMake library target first, then the two shims using the commands in
[reproduce.sh](probe-sources.tar.gz). Run the Rust binaries with these feature selections:

```sh
cargo run --manifest-path probe/Cargo.toml --release --offline --no-default-features --bin generator
cargo run --manifest-path probe/Cargo.toml --release --offline --bin superscalar_exec
cargo run --manifest-path probe/Cargo.toml --release --offline --bin portable_aes
cargo run --manifest-path probe/Cargo.toml --release --offline --bin vm
cargo run --manifest-path probe/Cargo.toml --release --offline --features full-probe --bin fullhash -- --large-key
```

The absolute paths in the probe manifest and build script refer to this review
workspace. The source snapshot is pinned by the source-digest manifest above.
