# Rejected forced superscalar inlining

Date: 2026-07-28 UTC

Adding `#[inline(always)]` to `ScProgram::execute` was tested against accepted
checkpoint `a110a61`. It preserved the real-block output but did not change
the SP1 cycle count:

```text
control:    6,684,639,305 cycles
candidate:  6,684,639,305 cycles
delta:      0
hash:       043f95d6e612d7c96879dd25ab78456481cfbb630143a5201c38920000000000
exit:       0
```

The candidate passed the 65,536-case superscalar differential before the SP1
measurement. LLVM's generated layout changed, but forced inlining supplied no
execution benefit. The annotation was reverted.

```text
f4f2776c52ccd3c3352efe2597cf66207a890064b847c33ddf82aa4ee174cf01  artifacts/randomx-real-unchecked-address-candidate       (291336 bytes)
9745b8d1cc145faafc9fb0ab8536dbf836555139a3068943f90fb12395124b5a  artifacts/randomx-real-inline-superscalar-candidate      (291336 bytes)
```

Every command used a 55-second hard timeout. No proof or paid proving-network
request was made.
