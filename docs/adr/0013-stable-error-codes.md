# Every `AdminError` maps to a stable code

**Status:** accepted.

Library errors used to reach clients as `SpelError::Unauthorized`, LEZ program error 1008, so a client could only tell them apart by message text. Each `AdminError` now maps to `SpelError::custom(code, message)` with a fixed code in the 1xx block, which LEZ reports as program error `6000 + code` (6101 to 6116 today). The message stays.

freeze-authority uses the 2xx block for `FreezeError`.

## Considered Options

**1. A code per variant, 1xx (chosen).**

**2. Keep `Unauthorized` with a message.** Rejected: clients have to match strings.

## Consequences

- Codes are API. A variant keeps its number for good, new variants take the next free one, and `codes_are_stable` pins the table.
- Framework errors the gate raises before library code runs, such as a missing signature on `#[account(signer)]`, keep SPEL's own codes.
