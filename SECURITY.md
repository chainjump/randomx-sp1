# Security policy

## Supported releases

Only the latest tagged release is supported. Unreleased commits and generated
artifacts not attached to a release are development material.

### Known correctness defect in v0.1.0

Release `v0.1.0` contains incorrect superscalar `IMULH_R` and `ISMULH_R`
register-selection metadata. The correction and regression test are committed
in `48e096823fd332076c2b5ab0e272beee27b2b473`. No tagged release or rebuilt SP1
guest containing that fix has been validated in this review. Do not use the
old release or its dependency revision for a new production deployment.

The [review record](evidence/randomx-review-2026-09-05/README.md) distinguishes
the confirmed generator defect from the remaining noncritical differences
and the validation needed for a funds-at-risk deployment. No per-key or
per-hash failure rate was established.

## Reporting a vulnerability

Do not disclose a suspected vulnerability in a public issue. Use GitHub's
[private vulnerability-reporting form](https://github.com/chainjump/randomx-sp1/security/advisories/new).
If that facility is unavailable, contact the repository owner through a
private channel before sharing technical details.

Include the affected commit or tag, target architecture, input needed to
reproduce the problem, and whether the issue affects native execution, SP1
execution, proof generation, or verification.

Never include prover credentials, private keys, or funded-account details in a
report or test fixture.
