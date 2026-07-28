# Rejected boxed superscalar slices

Date: 2026-07-28 UTC

After release-only metadata stripping, the immutable outer program vector and
each decoded executable vector were converted to boxed slices. This removed
their unused capacity words and reduced `ScProgram` from 32 to 24 bytes.

The cache/program-construction probe improved slightly, but the repeated
dataset path regressed enough to make the complete hash slower:

| Fixture | 32-byte `Vec` control | 24-byte boxed candidate | Change |
|---|---:|---:|---:|
| Cache/program construction | 5,159,976,799 | 5,159,969,591 | -7,208 |
| Real block | 6,684,502,428 | 6,684,527,987 | +25,559 regression |

The likely cause is visible in generated RV64IM addressing: the accepted
32-byte program stride is one shift, while a 24-byte stride needs an extra
operation in the hot dataset loop. The candidate retained the exact expected
cache bytes and real-block hash and exited successfully, but single-hash
cycles are the acceptance metric, so it was reverted.

```text
d0a239644d041a047a79f6a17af3b08a4bf2e74eed3a198f61dbc300bc646a70  artifacts/randomx-cache-stripped-metadata-candidate   (210520 bytes)
92e5d2dc243e47d0d226cef10dae44fc4d0f8bf98879775c303159e36f6ffc00  artifacts/randomx-cache-boxed-slices-candidate        (210744 bytes)
710b98f858574b42c74fafe811fcf3b8e06e00960ff47d5afa0d753ab7400e75  artifacts/randomx-real-stripped-metadata-candidate    (289304 bytes)
7f58c0ffe039afd24394e5a428fb386092aa6759adf47df9ca5a020baa12e59b  artifacts/randomx-real-boxed-slices-candidate         (289520 bytes)
```

Every command used a 55-second hard timeout. No proof or paid proving-network
request was made.
