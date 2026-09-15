# ADR-0011: Derived slot offsets

Status: accepted (derived-offset slice, 2026-08-05)

## Context

Embedded mode required the consumer to state the slot layout twice: the
`#[admin_slot]` field marker derived an `ADMIN_SLOT_OFFSET` const with a
layout test, and the module marker declared the same number by hand
(`offset = 32`), with a compile-time agreement assert catching drift
between the two. The declaration was redundant the moment the marker
existed, and every layout change meant editing a number a machine
already knew.

A proc macro cannot compute the number itself. At expansion time types
are tokens: aliases are unresolved, cross-crate types are opaque, and
any macro-side size table would produce silently wrong offsets for
whatever it misjudges. Only rustc knows the layout.

## Decision

The marker's `offset` kwarg becomes optional. A role without one is a
derivation, resolved after discovery by binding the role to the one
struct carrying its `*_slot` field marker and lowering the offset to
that carrier's const path (`ProgConfig::ADMIN_SLOT_OFFSET`). Emission
sites interpolate the path and rustc evaluates it, exactly as it always
evaluated the agreement assert.

The representation makes the pipeline explicit: an offset is
`Literal(n)`, `Derived`, or `Path(String)`. Parse produces the first
two, the resolution pass (`extension::slots::resolve_derived_offsets`)
rewrites `Derived` to `Path`, and every consumer after the pass treats
a surviving `Derived` as a hard error naming the missing pass, never a
silent skip. All three producers run the pass: the program macro, the
generate_idl macro, and the core IDL entry the CLI uses.

Recorded consequences:

- **Explicit offsets stay supported.** A declared `offset = <bytes>`
  keeps its agreement assert against the derived const. Derived markers
  have nothing to disagree with and get none.
- **Window collisions defer to rustc when derived.** Discovery rejects
  literal pairs sharing an account and offset; pairs involving a
  derivation become `const _: () = assert!(A != B)` in the consumer's
  crate, since the numbers exist only after layout.
- **The binding scan is the consumer's own code.** The entry file, its
  followed modules, and local path-dependency crates. Git and registry
  dependencies never participate: a foreign crate must not satisfy or
  steal the consumer's slot binding.
- **The carrier must be nameable at the consumer's root.** The lowered
  path names the carrier struct unqualified. A carrier bound from a
  local path dependency must be visible under that name where the
  program lives, or the emitted const path will not resolve. Trybuild
  style harnesses that path-depend on the very crate under test can
  poison the binding this way, which is why the negative fixtures live
  in the framework's hermetic e2e programs instead.
- **The cross-marker bound arg follows automatically.** Freeze's
  `admin_offset` (ADR-0012 in the freeze repository) receives the
  literal or the admin carrier's const path with no change on the
  freeze side.

## Consequences

The consumer states the layout once, on the field. Gate macros accept
the offset as an expression rather than an integer literal, mirroring
`#[admin_initialize]`, since the stamped value may now be a const path.
The samples drop their offset kwargs; the dry-run captures remain byte
identical, which is the whole point, derivation changes what the
consumer writes and nothing else.

Explicitly out of scope here: deriving the embedding account itself
(the `admin_config = config` kwarg) from the `#[admin_initialize]`
anchor. Designed, deliberately deferred: it inverts embedded-mode
detection, and its rules deserve their own record when built.
