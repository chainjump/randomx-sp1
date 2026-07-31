# Changelog

All notable changes to `randomx-sp1` will be recorded here.

The format follows Keep a Changelog, and releases use semantic versioning.

## Unreleased

- Separated the reusable ELF/vkey identity from the single-block proof
  evidence in the repository layout and documentation.
- Added verifier-ready proof and public-value data for independent
  Ethereum-mainnet verification with an external `cast call`.

## 0.1.0 - 2026-07-30

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
