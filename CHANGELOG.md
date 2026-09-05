# Changelog

All notable changes to `randomx-sp1` will be recorded here.

The format follows Keep a Changelog, and releases use semantic versioning.

## 0.1.1 - 2026-09-05

- Updated network-client `h2` and `chacha20` dependencies; documented the
  unreachable `lru::LruCache::pop` advisory and pinned the corrected guest.
- Fixed superscalar `IMULH_R` and `ISMULH_R` register-selection metadata to
  match RandomX v1.2.3, preserving random group parameters during source selection.
- Recorded canonical and adversarial host review and remaining differences.
- Reproduced the corrected SP1 ELF in two clean Docker builds and matched
  canonical RandomX in all 49 executions of that exact guest.
- Generated a fresh mainnet Groth16 proof and verified it locally and through
  the Ethereum-mainnet gateway, with invalid-input rejection checks.
- Published the corrected ELF/vkey and proof bundle; retained `v0.1.0`
  artifacts as historical evidence.

## 0.1.0 - 2026-07-31

- Added a dependent-program Quickstart and made the downstream ELF, vkey, and
  proof-verification boundary explicit.
- Separated the standalone ELF/vkey identity from the single-block proof
  evidence in the repository layout and documentation.
- Added verifier-ready proof and public-value data for independent
  Ethereum-mainnet verification with an external `cast call`.
- Added a self-contained release-asset checksum manifest and recovery record.
- Finalized the single-function `randomx_sp1::hash` consumer API.
- Added the universal SP1 guest with runtime RandomX key and blob inputs.
- Ported the applicable canonical RandomX v1.2.3 corpus and real Monero block
  fixtures.
- Added reproducible SP1 6.3.1 ELF and vkey evidence.
- Generated and locally verified a Succinct Prover Network mainnet Groth16
  proof for Monero block 3,727,837.
- Simulated and passed canonical Ethereum-mainnet verifier execution with
  `eth_call`; no transaction was broadcast.
- Added release, CI, security, contribution, provenance, and attribution
  documentation.
