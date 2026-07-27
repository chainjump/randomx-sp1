# Clean isolated baseline

Date: 2026-07-27 UTC

Source checkpoint:

```text
commit       426fcd2fb504e59cacd2f8e820e82c18421f35d7
tree         c1934ab9efaa72d8c2be516a4b17dda94b3bce0a
Cargo.lock   133a760654a93015ac7ea8b52e871c6043ae8a79bb88e5b728cd55f4df4f27d2
```

Both guests were rebuilt from the isolated repository with the required
bounded command:

```text
timeout --signal=INT --kill-after=1s 55s cargo prove build --locked ...
```

The first cold build reached the watchdog while compiling dependencies and was
resumed with the same bounded command. No individual command exceeded 55
seconds.

## Real-block fixture

```text
artifact: artifacts/randomx-real-baseline
size:     293232 bytes
sha256:   62bf3ee459f1db9d65fdc87d8b0601e817ba937206d1ef971d9498162b2c7d42
cycles:   6776439804
output:   043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
exit:     0
wall:     13.33 seconds
```

Execution command:

```text
timeout --signal=INT --kill-after=1s 55s \
  /root/experiment/target/release/execute-fast \
  artifacts/randomx-real-baseline \
  043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
```

## CFROUND-heavy fixture

```text
artifact: artifacts/randomx-cfround-baseline
size:     293280 bytes
sha256:   4f0926132bb6748e2df2679dc3a1968814bd995ffbe21d433baf312eb4befcaa
cycles:   6969759319
output:   c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
exit:     0
wall:     13.45 seconds
```

Execution command:

```text
timeout --signal=INT --kill-after=1s 55s \
  /root/experiment/target/release/execute-fast \
  artifacts/randomx-cfround-baseline \
  c19ae2f2f50a2e33ec737484e6c447d9b0ffe44431a33201026ba9eca70fda95
```

The CFROUND-heavy cycle count and output exactly reproduce the latest handoff
record. These are lightweight executor measurements, not an SP1 proof or PGU
measurement.
