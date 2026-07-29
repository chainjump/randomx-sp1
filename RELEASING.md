# Release process

1. Finish all source, manifest, naming, and documentation changes.
2. Run every command in `CONTRIBUTING.md` and audit both lockfiles with
   `cargo audit`.
3. Commit the final source and require CI to pass on that commit.
4. From `program/`, build the guest with the pinned production command:

   ```bash
   cargo prove build --docker --tag v6.3.1 --locked \
     --elf-name randomx-sp1-program \
     --output-directory ../artifacts
   ```

5. Record the source commit, `Cargo.lock` hash, Docker image digest, ELF size,
   SHA-256, SHA-512, disassembly review, and locally derived vkey.
6. Execute that exact ELF against the selected Monero block and record its
   public output, cycle count, and PGU estimate. Adjust proof limits only from
   the measured result.
7. Record the approved ELF SHA-256 in the fail-closed prover client and rerun
   its offline tests.
8. Obtain explicit approval before making any paid prover-network request.
9. Save the request ID immediately, verify the returned proof locally, and
   simulate verification with an Ethereum-mainnet `eth_call`. Do not broadcast
   an EVM transaction.
10. Commit the artifact and complete evidence, require CI to pass, create a
    signed `v0.1.0` tag, and push the commit and tag to the release remote.

The CI workflow validates code and dependencies but intentionally does not
build a release ELF, access secrets, execute the multi-billion-cycle guest, or
submit a proof. Those steps are explicit release operations because they bind
an exact source commit and can consume substantial resources or funds.
