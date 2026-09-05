# Corrected RandomX production validation

Guest source: `5b18879863d140d7ae1aaa25fb2534da4bf89de4`, containing the
superscalar correction from `48e096823fd332076c2b5ab0e272beee27b2b473`.
The corrected standalone guest passed reproducible builds, canonical guest
execution, live network proving, local verification, and Ethereum-mainnet
verification. Release `v0.1.1` packages this identity and proof.

## Build and execution

Two independent clean SP1 6.3.1 Docker builds produced the same 289,528-byte
ELF, SHA-256 `a2c35c9e93f6bf4d891be3d21ad22caa34b6e710805f2e634c246aaa6a1b3884`.
The image digest is
`sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400`.
[Build records](builds.json), [tool versions](build-environment.json), and
[guest source digests](guest-source-sha256.json) bind the artifact to its
inputs. Later release changes leave these guest inputs unchanged. The
[ELF review](elf-review.json) and [headers](elf-headers.txt) identify an RV64IM
integer-only executable, with all 20 `ecall` sites inside SP1's halt, hint,
and public-output functions.

All **49 executions of that exact ELF** matched freshly calculated canonical
C++ RandomX v1.2.3 outputs. The corpus covers eight keys and six blobs, plus
Monero block 3,727,837. Keys include lengths 0, 1, 12, 32, 60, 64, and 257;
blobs include empty input and lengths through 4,096 bytes. The 64- and
257-byte keys are extension coverage outside the specified 0–60-byte domain.
The oracle revision and full inputs are in [guest-vectors.json](guest-vectors.json);
all outputs, exit codes, and cycles are in [guest-executions.json](guest-executions.json).

The exact guest matched the canonical RandomX result for Monero block
3,727,837 in 6,447,168,673 cycles and 7,797,851,749 calibrated PGU. See
[execution and gas output](gas-estimate.txt). The configured request limits
remain 6,500,000,000 cycles and 8,000,000,000 PGU; they apply to this fixed
block. Some other corpus inputs exceed that cycle limit.

The host release suite passed 45 tests, with one intentionally ignored deep
audit; the network client passed seven tests. Formatting, feature-specific
Clippy, documentation, the exact-revision consumer, and dependency audits
passed. See [validation.json](validation.json), [workspace tests](test-workspace.txt),
[client tests](test-client.txt), and [dependency audits](dependency-audits.txt).
The client's two `lru` advisory exceptions concern unreachable APIs and are
explained in [its README](../../network-prover/README.md). CI passed on
[`66d1f39`](https://github.com/chainjump/randomx-sp1/actions/runs/33941470183)
before the paid request; the release also requires CI on the final artifact commit.

## Live proof and verification

| Item | Result |
| --- | --- |
| [Program vkey](../../artifacts/v0.1.1/randomx-sp1-program.vkey) | `0x00360be15dc8e8f448b5a9ab15bb368ed6301842244316a7705380dce0501d66` |
| [Mainnet request](https://explorer.succinct.xyz/request/0xa4e4dbc495e25cedb2569e22fb4071bd11adc0393a504daced092a2dbe1c6a83) | Fulfilled in 196 seconds on 2026-09-05 |
| [Public RandomX digest](public-values.hex) | `5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000` |
| [SDK proof](proof.bin) | 1,725 bytes; SHA-256 `e144061e4835e92df22130c5b012004b8cca524fd0fd45c01dd39841a63d348f` |
| [EVM proof](proof-evm.hex) | 356 decoded bytes |
| Local verification | SP1 SDK 6.3.1 verified against the vkey derived locally from the exact ELF; public bytes matched the canonical hash |
| Portable verification | SP1 6.3.1 Groth16 verifier accepted the proof and rejected the old vkey, changed digest, six proof mutations, and empty proof |
| Ethereum verification | Canonical Groth16 gateway accepted the proof through two RPC providers at finalized block 25,908,376 |

The [network receipt](network-receipt.json) reports exactly the locally
measured cycles and PGU. The request cost was capped at 6.736666 PROVE;
settlement was still pending when this receipt was captured, so it does not
claim a final charge. Its raw `vk_hash` serializes eight 32-bit field words;
`program_vkey` packs those same words at 31-bit spacing for Solidity. They
identify the same key. The network's `sp1-v6.1.0` circuit version is the one
used by SDK 6.3.1.

[Proof identity](proof-identity.json), [portable verification output](portable-verification.txt),
and [Ethereum RPC responses](evm-verification.json) retain the results.
Reloading the saved SDK proof also reproduced the exact EVM bytes and public
values and confirmed the SDK's expected circuit version.
PublicNode and dRPC agreed on chain ID 1, the finalized block hash, and the
gateway bytecode. Both returned `0x` for the valid call and reverted for the
historical vkey, changed public digest, and changed proof. No EVM transaction
was signed or broadcast. The calls use Succinct's
[documented gateway](https://docs.succinct.xyz/docs/sp1/verification/contract-addresses)
and [verifier interface](https://docs.succinct.xyz/docs/sp1/verification/solidity-sdk).

## Reproduce and verify

From the repository root, with Docker and SP1 CLI 6.3.1 installed:

```bash
rebuild_dir="$(mktemp -d)"
git worktree add --detach "$rebuild_dir/source" \
  5b18879863d140d7ae1aaa25fb2534da4bf89de4
(
  cd "$rebuild_dir/source/program"
  cargo prove build --docker --tag v6.3.1 --locked \
    --binaries randomx-sp1-program --elf-name randomx-sp1-program \
    --output-directory "$rebuild_dir"
)
cmp artifacts/v0.1.1/randomx-sp1-program "$rebuild_dir/randomx-sp1-program"
cargo prove vkey --elf "$rebuild_dir/randomx-sp1-program"
```

The vkey must equal the value above. Each recorded build used a separate
clean worktree and target directory. Check the retained files with:

```bash
(cd artifacts && sha256sum --check SHA256SUMS)
(cd evidence/production-2026-09-05 && sha256sum --check SHA256SUMS)
```

For verification without repository code, download every `v0.1.1` release
asset into an empty directory, enter it, and use Foundry `cast`:

```bash
sha256sum --check SHA256SUMS
export EVM_RPC_URL='<ethereum-mainnet-rpc-url>'
test "$(cast chain-id --rpc-url "$EVM_RPC_URL")" = '1'
cast call 0x397a5f7f3dbd538f23de225b51f532c34448da9b \
  'verifyProof(bytes32,bytes,bytes)' \
  "$(tr -d '\r\n' < randomx-sp1-program.vkey)" \
  "0x$(tr -d '\r\n' < public-values.hex)" \
  "0x$(tr -d '\r\n' < proof-evm.hex)" \
  --rpc-url "$EVM_RPC_URL"
```

Success returns `0x`. [evm_verify.py](evm_verify.py) reproduces the two-provider
checks with negative controls; it needs Python 3, `curl`, and `cast`.
[verify-proof.rs](verify-proof.rs) contains the portable local checks; its
recorded build command reused the locked SP1 6.3.1 host dependencies.

## Scope

The [focused host review archive](focused-host-review.tar.gz) preserves the
preceding floating-point and memory-safety review, its probes, and results.
That review used isolated host programs and was performed by the same
assistant, not an independent auditor. Its original absolute temporary paths
and pre-deployment status are retained as historical context.

This completes validation of the corrected library's standalone guest.
Finite comparisons and one proof do not establish correctness for every
input. A consuming financial application must bind its intended key, blob,
and result in its own statement and validate its complete guest and vkey;
this standalone guest exposes only the digest.
