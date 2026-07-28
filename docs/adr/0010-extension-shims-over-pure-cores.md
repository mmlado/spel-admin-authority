# Extension instructions split into consumer-emitted shims over pure library cores

Status: Proposed. Not yet reviewed by the maintainers. Date: 2026-07-28.

Would supersede: freeze-authority ADR-0012 (cross-marker bound args, shipped in M2.5) as a permanent mechanism.
Would amend: ADR-0009 instruction-drift-shared-file (drift mechanism replaced), ADR-0008 extensions-self-emit-scanner-removed (confirmed and extended), ADR-0007 (bootstrap stays consumer-explicit, an emission-time presence check is added).
Relates to: spel ADR-0002 (attribute ordering inverts under self-emission), freeze ADR-0004 and ADR-0007 (semantics preserved unchanged).

## Context

Three physical constraints shape every option in this space. A gate attribute on a library fn expands when that library compiles, so nothing stamped at consumer expansion can reach it. A proc macro running at consumer expansion cannot read the extension crate's source, so extension knowledge must be captured when the extension itself builds. Extension information can live in exactly three places, macro input, source plus manifest text, or compiled artifact, and each is readable by a different kind of tool.

The LEZ platform adds its own rules. A transaction naming the same account id twice is rejected at admission. Execution must produce exactly one post-state per account, positionally paired with pre-states. Program identity is the hash of the bytecode, so a changed offset is a different program.

The project's hard requirements are third-party extensibility with no framework change per extension, and a single source of truth for each instruction surface.

The immediate trigger was freeze-authority's embedded adoption. Its transfer and renounce need admin-authority's slot location, and every courier mechanism for moving that location into a precompiled freeze body either leaked a silent default, depended on metadata ordering, or existed only to work around expansion locality.

## Decision

Every extension instruction splits into two artifacts with a hard boundary.

A core fn compiled in the extension library. Pure, no attributes, no gates, no SpelOutput. It takes explicit window-typed location arguments and, where it needs another extension's state, an already decoded read-only view. The existing `_at` family is the template for this convention.

A shim generated into the consumer module by `apply_extension` from a typed per-instruction manifest, via the attribute macro that `spel_extension!` produces. The shim owns account params and constraints, gate attributes, location values baked as literals, and SpelOutput assembly. Because the shim is real consumer-crate code, gates expand where every marker, offset, and peer declaration is known.

Required hardenings, adopted together with the split:

- Each extension attribute re-emits an inert breadcrumb carrying its consumed declaration and its source crate identity, checked against the followed use import. Marker ordering violations are hard errors with a guided message. A manifest-declared foreign role that resolves to neither a live marker nor a breadcrumb is a hard error. Dedicated fallback is permitted only for a marker that is present and bare, never for an absent one.
- Extension libraries export layout constants (slot size, footprint, dedicated seed). `apply_extension` emits consumer-compiled const assertions against those constants for window bounds, window overlap between extensions, and agreement between the marker offset and the layout-derived field offset.
- Offsets are typed per config type, so transposing two extensions' offsets is a type error, not a runtime misread.
- The core ABI uses a window-restricted mutable view fusing account and offset, so a core cannot write outside its declared window.
- A selfcheck macro instantiated in the extension library, backed by a sentinel constant that generated shims reference, plus a conformance harness expanding a synthetic consumer per configuration cell in the extension's own CI.
- Any instruction touching an init role must carry an explicit embedded disposition in the manifest. Absence is a hard error, never a silent emission.
- Roles and gates are extension-qualified. Shims are never default-wrapped by foreign extensions, only by gates their own manifest declares.

## Rejected alternatives

1. Scanner status quo extended with stamp-but-never-emit and direct cross-crate dispatch. Stamped kwargs cannot reach gate bodies compiled in the library, and three representations of each instruction drift by discipline alone.
2. Per-extension statics registering declarations in the compiled library. Readable only by a full compile, invisible to replay IDL generation and to consumer expansion, breaks producer identity.
3. An offsets map keyed by module name. Stringly keys with no crate identity, a second source of truth beside the markers, open to name squatting.
4. Cross-marker bound_args feeding shared precompiled bodies, the shape freeze ADR-0012 specified. A load-bearing default of zero silently conflates a dedicated peer with an absent peer, metadata order can transpose same-typed values undetectably, and the whole mechanism serves only expansion locality, which this decision removes.
5. Embedded fn variants selected by a shape table. Variant symbols leak into ordinary discovery as phantom instructions, attribute parity between variants is unenforced, and the selection grammar cannot express nested partitions.
6. Prohibiting distinct-account embedding. Breaks dedicated mode as the degenerate case and breaks mixed mode.
7. Prohibiting same-account embedding. Bans the natural single-state-account layout, defers the failure to runtime admission, and forfeits the transaction-size win.
8. Role-unified body rewriting, where the extension macro substitutes idents inside authored bodies and merges execute vectors. Requires a binding resolver over third-party token streams and fails by compiling wrong rather than loudly. Its breadcrumb and ordering-guard ideas are adopted here.
9. Auto-injecting the bootstrap call into the consumer's init instruction. No sound insertion point exists relative to the consumer's own writes, and the silent failure mode is a legal all-zeros renounced slot on an immutable program. The consumer-explicit `bootstrap_at` from ADR-0007 stays, with a new emission-time presence check.
10. The previously rejected delegation thin-wrapper, re-answered. The selfcheck plus sentinel moves interface verification to the extension's CI and the first consumer build. This is weaker than compiling the same bytes twice and is recorded as an accepted cost.
11. Macro expansion via RUSTC_BOOTSTRAP for IDL generation. Unstable dependency, compiles the whole graph anyway. Replay from the manifest stays the IDL path.
12. A closed emission library compiled into the CLI. Fails the third-party extensibility requirement.

## Consequences

The drift guarantee changes class, from same-bytes inclusion to a manifest-to-core interface held by selfcheck, sentinel, and conformance harness. The manifest grammar becomes owned, versioned surface. Migration changes every program id, which is inherent to LEZ. Writing an extension now includes running the conformance harness.

The aliasing hazard is deleted at the root. A shared embedding account is bound once, in the shim, and SpelOutput assembly emits one post-state per unique account, so the same-account layout is fully supported instead of forbidden.

Value-level semantics are untouched. Dual-path freeze renounce, vacate rather than terminate, terminal admin renounce, self-election, born-initialized embedded slots.

Open items, each a maintainer call or a follow-up ADR. Mixed mode and distinct-account-separate-inits stay compile errors until their own ADR resolves the bootstrap-ordering window. The silent attribute-drop diagnostic becomes a prerequisite before the first slice. The `generate_idl!` path either retires or hard-errors on distributed extension crates. The duplicated JSON renderer unifies into spel-framework-core. A research item probes removing the remaining cargo metadata use in spel-cli.
