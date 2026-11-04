This release fixes superscalar `IMULH_R` and `ISMULH_R` register-selection
metadata to match canonical RandomX v1.2.3. The earlier `v0.1.0` guest predates
the fix; use the new ELF and verification key together.

- Two clean SP1 6.3.1 Docker builds produced the same corrected ELF.
- All 49 executions of that ELF matched canonical C++ RandomX outputs.
- The host release suite passed 45 tests and the network client passed seven.
- A fresh mainnet Groth16 proof passed local verification and the canonical
  Ethereum-mainnet gateway through two RPC providers. Incorrect keys, public
  values, and proof bytes were rejected. No EVM transaction was broadcast.
- The network client updates `h2` and `chacha20`; two reviewed, unreachable
  `lru` advisory exceptions are documented in its README.

Guest source: `5b18879863d140d7ae1aaa25fb2534da4bf89de4`.
ELF SHA-256: `a2c35c9e93f6bf4d891be3d21ad22caa34b6e710805f2e634c246aaa6a1b3884`.
Program vkey: `0x00360be15dc8e8f448b5a9ab15bb368ed6301842244316a7705380dce0501d66`.

[Live proof request](https://explorer.succinct.xyz/request/0xa4e4dbc495e25cedb2569e22fb4071bd11adc0393a504daced092a2dbe1c6a83).
The [production record](https://github.com/chainjump/randomx-sp1/blob/v0.1.1/evidence/production-2026-09-05/README.md)
contains source reproduction, all verification results, and the standalone
Ethereum `cast call` command.

Download all release assets into an empty directory and run
`sha256sum --check SHA256SUMS`. The bundle contains the ELF, vkey, SDK proof,
EVM proof bytes, public values, request ID, and checksum manifest.

This validates the standalone RandomX guest. A consuming application needs
its own statement binding, complete guest identity, and proof validation.
