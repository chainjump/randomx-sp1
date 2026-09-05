# Corrected RandomX production validation

Guest source: `5b18879863d140d7ae1aaa25fb2534da4bf89de4`, containing the
superscalar correction from `48e096823fd332076c2b5ab0e272beee27b2b473`.

Two independent clean SP1 6.3.1 Docker builds produced the same 289,528-byte
ELF, SHA-256 `a2c35c9e93f6bf4d891be3d21ad22caa34b6e710805f2e634c246aaa6a1b3884`.
The image digest is
`sha256:0942a27dbe8e38f4b14f3732e779df4027b17bde93e9fbc9e8c773c15eb63400`.
[Build records](builds.json) and [guest source digests](guest-source-sha256.json)
bind the artifact to its inputs. The ELF review is recorded in
[elf-review.json](elf-review.json) and [elf-headers.txt](elf-headers.txt).

The exact guest matched the canonical RandomX result for Monero block
3,727,837 in 6,447,168,673 cycles and 7,797,851,749 calibrated PGU. See
[execution and gas output](gas-estimate.txt). The configured request limits
remain 6,500,000,000 cycles and 8,000,000,000 PGU; they apply to this fixed
block, not to arbitrary inputs.

The remaining guest corpus, network proof, local verification, and Ethereum
mainnet verifier demonstration are still in progress. This is not yet a
completed release record.

The [focused host review archive](focused-host-review.tar.gz) preserves the
preceding floating-point and memory-safety review, its probes, and results.
That review used isolated host programs and was performed by the same
assistant, not an independent auditor. Its original absolute temporary paths
and pre-deployment status are retained as historical context.
