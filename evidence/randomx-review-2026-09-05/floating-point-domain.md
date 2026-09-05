Floating-point domain review, 2026-09-05

This argument concerns values passed to floating-point arithmetic during one
RandomX VM iteration, with default RandomX v1 parameters. The final F XOR E
mixing can produce arbitrary bit patterns, but those values are serialized or
stored as bits. All F and E lanes are initialized again before the next
iteration's arithmetic. Lane swaps do not change these bounds.

Canonical sources: [specification](https://github.com/tevador/RandomX/blob/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab/doc/specs.md),
[branch design](https://github.com/tevador/RandomX/blob/12f2c2ffe2108d6cf54c391fee33c8bc3646cdab/doc/design.md#262-cbranch-instruction).
The specification expressly excludes NaNs and denormal arithmetic results;
the argument below checks how the reviewed implementation preserves that
restriction. It is a source-level mathematical argument, not a mechanically
verified equivalence proof.

Branch execution bound

CBRANCH chooses the instruction after the last write to its destination as
its target. Every CBRANCH marks every integer register as modified. Therefore
its repeated region contains neither a write to the controlling register nor
another branch. The repeated regions cannot nest or cross a preceding branch.

Let b be the selected condition bit, 8 <= b <= 23. The increment has bit b set
and bit b-1 cleared. Split it into a low-b-bit value c and a condition-byte
increment q. Then c < 2^(b-1) and q is odd. Once the branch has been taken,
the condition byte is zero. A subsequent addition can leave it zero only
when q = 255 and the low-b-bit addition carries. Two successive additions of
c cannot both carry: after a carry, the low value is less than c, so the next
sum is less than 2c < 2^b. Consequently at most two consecutive branches are
taken, and every instruction executes at most three times per iteration.

[integer.log](deep/integer.txt) also records an exhaustive check of 1,073,741,824
reduced transition combinations: all 256 low-byte values, all 128 permitted
low-byte increments, all 128 odd condition-byte increments and all 256
initial condition bytes. The proof above extends the carry argument to each
of the actual b values; the finite enumeration alone is not that proof.

E lanes

The initial E mask and the memory-divisor mask produce positive normal
values in [2^-255, 2). A lanes are fixed positive finite values in [1, 2^32).
Only multiplication by A, division by a masked memory operand, square root,
and lane swapping modify E during instruction execution.

For values below 1, square root cannot reduce E. Multiplication by A cannot
reduce it either. A division can reduce E by at most a factor of two. This
holds for every rounding mode while E/2 is representable, since E/2 is a
representable lower bound on the exact quotient and on its rounded result.

A program with a branch has at most 255 other instructions, each executing
at most three times; hence it has at most 765 divisions affecting a lane.
A program without a branch has at most 256. The conservative minimum is
therefore 2^(-255-765) = 2^-1020, above the smallest normal value 2^-1022.
There is no zero divisor, underflow to zero, or subnormal E result.

Overflow to positive infinity is permitted. Once it occurs, multiplication
by positive finite A, division by positive finite memory operands, and square
root preserve positive infinity. No E operation subtracts infinities,
multiplies infinity by zero, or takes a negative square root.

F lanes

F starts as signed i32 values. Arithmetic adds or subtracts either a fixed
A lane or a signed i32 memory operand. A nonzero source has magnitude at least
1 and less than 2^32. Zero memory sources leave the magnitude unchanged.

An addition or subtraction involving a nonzero source can have magnitude
below 1/2 only by cancellation with an F operand of magnitude at least 1/2.
Both operands in such a cancellation are multiples of 2^-53: binary64
spacing at magnitude >= 1/2 is at least 2^-53. The nonzero exact residual
is therefore at least 2^-53, and rounding cannot make it a subnormal.
When there is no such cancellation, the result has magnitude >= 1/2.

FSCAL XORs the sign and the low four exponent bits. For a normal nonzero
number it changes the exponent by at most 15. Consecutive FSCAL operations
cancel; lane swaps and additions of zero do not create another reduction.
Thus a nonzero arithmetic result followed by FSCAL stays at least 2^-68.
The separate zero case is safe too: FSCAL transforms signed zero into a
normal value of magnitude 2^-1008 with a zero fraction. Another FSCAL restores
zero. Addition of zero preserves that value; addition of a nonzero source
returns to the ordinary arithmetic case. Hence no subnormal F operand or
result is reached.

To exclude FSCAL mapping an enormous exponent into a NaN, the accompanying
[interval analysis](float-domain-bound.py) tracks conservative binary64
intervals for each exponent binade. It begins with the larger continuous
range [0, 2^31], permits any real signed source of magnitude <= 2^32, rounds
interval endpoints outward, and applies FSCAL's exponent mapping exactly.
These choices overapproximate possible F values; they cannot establish the
lower bound above, but they can establish an upper bound.

After 768 arbitrary instruction effects, including arbitrary combinations
of scale and addition/subtraction, the computed upper bound is
`0x1.000000000001ap+432`. No intermediate interval reaches exponent 2047.
This is deliberately loose, but sufficient to exclude infinity and NaNs in
F arithmetic. It covers the maximum three executions of each of 256
instructions. Results are in [float-domain-bound.log](deep/float-domain-bound.txt).

Directed-arithmetic integer bounds

A finite normal binary64 magnitude has a <=53-bit integer coefficient.
Products of two coefficients occupy at most 106 bits. The multiplication,
division and square-root exact-versus-nearest comparisons align such
products with a nearby binary64 result; their aligned integers fit in u128.
Exact cancellation and signed zero have separate paths.

Addition/subtraction align coefficients directly only for exponent distances
<=75. At the maximum distance, the largest same-sign sum is bounded by
(2^53-1)*2^75 + (2^53-1), which is below 2^128. At larger distances the smaller
operand is below half an ULP; the code uses its sign/tail to choose a directed
neighbor without performing a large shift. Infinity is intercepted before
finite-magnitude decomposition. These assumptions were also exercised with
targeted coefficient/exponent edges and all four rounding modes in the
pinned-runtime differential probe.
