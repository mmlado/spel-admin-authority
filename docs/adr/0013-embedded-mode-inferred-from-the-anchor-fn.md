# ADR-0013: Embedded mode is inferred from the anchor fn

Status: accepted (account-inference slice, 2026-08-13)

## Context

Embedded mode was declared on the module marker: a role kwarg naming the
embedding account (`#[admin_authority(admin_config = config)]`), with
the offset first declared by hand and then derived (ADR-0011). The kwarg
was the last piece of ceremony left, and it duplicated something the
program already said: the fn carrying `#[admin_initialize]` declares the
embedding account as its `#[account(init)]` param. Two declarations of
one fact invite drift, and the marker kwarg was the only reason the
marker could not stay bare across modes.

## Decision

The extension declares an anchor in its embedded metadata, `anchor_attr`
and `anchor_role` together (`admin_initialize` / `admin_config`), and an
anchored extension must declare `embedded.state_type` at authoring time.

A consumer fn carrying the anchor attribute puts the extension in
embedded mode. Its single `#[account(init)]` param is the embedding
account; with several init params the anchor's role kwarg names the one
(`#[admin_initialize(admin_config = config)]`, the same ADR-0010 kwarg
the inject contract reads). The module marker stays bare in both modes,
and the role kwarg is retired for anchored extensions: writing it is a
hard error, the anchor fn is the single writer. The offset is always
derived, so the anchor rides on the ADR-0011 machinery and there is no
offset kwarg either.

The anchor is also the declared initializer. Every instruction that
creates the embedding account must carry it, or the build refuses: the
coverage gate reads the declared attr, never a naming convention.

Inference runs at discovery time, not as a later pass, because
`embedded.skip` filters the instruction set inside discovery: an
inferred embed must drop the extension's initializer instruction exactly
like a kwarg-declared one did.

The slot field marker and the anchor must agree:

- A struct carrying `#[admin_slot]` with no fn carrying
  `#[admin_initialize]` is a hard error. The marked field declares
  embedded intent, nothing bootstraps the window, and the program would
  ship born renounced while compiling as dedicated mode. Discovery
  records the anchor-capable extension that resolved dedicated, and the
  dispatcher refuses when a slot carrier exists for it, naming the
  struct, both attributes, and both fixes.
- An anchored fn with no marked field is a hard error too: the
  derivation has no carrier, and the message offers the two fixes that
  exist, since an anchored extension has no offset kwarg to fall back
  to.
- No marked field and no anchored fn is plain dedicated mode.

The dispatcher enforces the agreement; the IDL producers do not. They
run without a carrier scan by design (admin ADR-0012's companion work in
the framework), so the check would see nothing, and the consumer's build
is the gate: a program that does not compile has no IDL worth
generating.

## Consequences

The consumer's embedded surface is three declarations, each on the thing
it describes: `#[admin_slot]` on the field that holds the slot,
`#[admin_initialize]` on the instruction that creates the account, and a
bare `#[admin_authority]` on the module. Nothing is declared twice.

The failure mode for a forgotten initializer changes shape. With the
slot field marked, it is a compile error naming the struct and both
attributes. Without the marker there is nothing declaring embedded
intent, and the program is dedicated mode by definition — the extension
manages its own Config PDA and the consumer's struct is plain data.

Two fns carrying the anchor is a hard error naming both. One fn carrying
the anchor without an `#[account(init)]` param is a hard error naming
it: the anchor fn creates the embedding account.

The `marker_offset_disagreement` compile-fail fixture is retired. The
scenario it pinned — a marker's literal offset disagreeing with the
derived const — is unrepresentable for an anchored extension, which has
no marker offset to disagree with. The agreement assert itself remains
in the framework for kwarg-declared extensions and is tested there.

The dormant-anchor check's end-to-end coverage is deliberately the
`missing_admin_initialize` fixture in this repo: the framework's unit
test hands items to the check directly, so only a fixture compiled
through the real macro exercises the scan wiring that feeds it.

## Considered alternatives

**Keep the marker kwarg.** Explicit, and no inference machinery.
Rejected: it restates what the anchored fn already declares, and two
writers of one fact is the drift ADR-0011 removed for the offset.

**Derive the initializer from a naming convention** (`<role minus
_config>_initialize`). No metadata needed. Rejected when the declared
initializer landed: an extension naming its initializer anything else
silently lost the born-renounced gate, and a guard that vanishes on a
rename is worse than none.

**Warn instead of refusing on slot/anchor disagreement.** Backward
compatible by construction. Rejected: the disagreement ships a born
renounced program, the framework fails closed everywhere else, and both
declarations sit in code the author owns.
