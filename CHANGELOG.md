# Changelog

All notable changes to `randomx-sp1` will be recorded here.

The format follows Keep a Changelog, and releases use semantic versioning.

## Unreleased

- Fixed superscalar `IMULH_R` and `ISMULH_R` register-selection metadata to
  match RandomX v1.2.3, preserving random group parameters during source selection.
- Documented the canonical and adversarial host review, remaining differences,
  and production-validation gaps; updated the dependency pin to the corrected
  source and marked the retained `v0.1.0` guest evidence as historical.

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
