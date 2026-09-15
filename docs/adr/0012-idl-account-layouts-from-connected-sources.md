# ADR-0012: IDL account layouts come from connected sources

Status: proposed (M3 follow-up, 2026-08-12)

## Context

`#[account_type]` marks an account layout, and `spel generate-idl` puts
every layout it finds into the IDL's `accounts` section. That section is
the catalogue `spel inspect` decodes against, and the list
`decode_account_data_try_all` walks when guessing an account's shape
from raw bytes with no type name.

Which crates get scanned for the annotation was widened during M3.
Before, it was the consumer's local path dependencies: a manifest walk,
no subprocess. M3 added a `cargo metadata` layer so extensions arriving
by git could be found, since `admin-authority` and `spel-authority` are
git dependencies and a path-only walk leaves `AdminConfig` unresolved.
The widening was necessary. What it also did was take the annotation
scan from "crates the author linked by hand" to "every normal-kind
package in the runtime graph".

Measured on a real consumer, that graph is 252 packages, of which 8
mention `account_type` at all: the consumer, admin-authority,
spel-authority and the framework crates. The other 244 are parsed in
full and contribute nothing. The same walk is why an unrelated crate's
sources reach the type collector, which is how the ark-ff lexer failure
arose.

Two consequences follow from scanning crates the program has no
connection to. `accounts` is a `Vec` with no dedup and `find_type_def`
takes the first match, so a crate anywhere in the graph declaring a
colliding name shadows an extension's layout for every lookup. And a
crate can carry the annotation without depending on the framework at
all, by defining its own proc macro of that name, since the attribute is
matched on the path's last segment. Both are client-side: ADR-0007
records that the IDL is paperwork the program never consults, so the
blast radius is an operator shown a wrong decoding, not a program
accepting a transaction it should refuse.

The symptom is already visible without an attacker. `AuthoritySlot`
carries the annotation in `spel-authority`, so it sits in `accounts` as
a decode candidate, while it is never stored as a whole account: it
lives inside `AdminConfig`, which lives inside the Config PDA or a
consumer's embedded window.

The same root shows up in three places, which is the strongest argument
for narrowing rather than patching. The ark-ff failure was this walk
lexing a registry crate that no program references.
`has_metavar_glued_literal` exists as a skip for source the walk cannot
lex at all, which is a workaround for opening a file that should never
have been opened. And because `syn` does not evaluate `cfg` while the
walk descends into `Item::Mod` bodies, an `#[account_type]` inside a
dependency's `#[cfg(test)]` module reaches a consumer's IDL;
`spel-framework-macros` has exactly that shape today. Each is the same
mistake, that source text somewhere in the dependency graph is part of
this program.

## Decision

An account layout reaches the IDL because the program is connected to
it, not because the annotation exists somewhere in the dependency graph.
Three sources are connected, and each is already computed during
discovery:

**Owned sources** — the consumer's crate and the local path
dependencies it links. The author wrote or deliberately linked that
code, and for a program's own layouts there is no other signal: an
account parameter is typed `AccountWithMetadata`, so nothing ties it to
the struct whose bytes it holds.

**Activated extensions** — the extension's `#[instruction]` functions
are merged into the consumer's dispatcher and ship in its binary, so the
accounts they create and write are the program's accounts. This holds
whether or not the consumer's own code names the type.

**Referenced types** — anything a type already in the IDL has a field
of, wherever it lives. `AuthoritySlot` arrives this way, and so does
`AdminCandidate`, whose alias target lives in a crate that is never
scanned.

Everything else is an unowned crate: its annotations are inert, and it
cannot put a layout in a program's IDL.

Resolving referenced types therefore stops being a by-product of
scanning everything and becomes demand-driven: the walk asks for a name,
and only then does anything go looking for the crate that declares it.

## Consequences

`AuthoritySlot` moves from `accounts` to `types`. Named decoding is
unaffected, because `find_type_def` searches `accounts` and falls back
to `types`. What changes is that it stops being a candidate in the
guess-by-shape loop, which is the point.

The collision surface closes by construction rather than by a check: an
unowned crate cannot contribute a layout, so it cannot shadow one.

Two account layouts of the same name are a hard error naming the type
and the path of each declaring crate. After the narrowing this can only
be reached by two owned sources, or an owned source and an activated
extension, colliding: code the author wrote or deliberately switched on,
which is why it is theirs to sort out rather than something to route
around. It also matches how the framework already treats duplicate
instruction names.

Refusing is what makes the check visible at all. A proc macro can emit a
compile error but not a warning, so a warning would appear only when
someone ran `spel generate-idl` and stay silent through every
`cargo build` — the moment a developer would actually notice. The
alternative was emitting a `#[deprecated]` shim to manufacture a rustc
warning, which is a mechanism the framework does not otherwise need.

This is a breaking change: a program that today ships two same-named
layouts builds, with the first winning every lookup, and will stop
building. The break is deliberate, because that program already has an
ambiguous IDL and no way to know it. It is called out here because the
delivery is otherwise backward compatible, and a reviewer may want to
weigh it separately.

No program in the admin or freeze trees trips it. Scanning all three
samples' graphs for declared layouts turns up eleven or twelve names
each and no collision among the crates the scan actually reads. The two
apparent hits are neither: sibling sample crates are workspace members,
which the walk skips, and the repeated `TokenHolding` is one real
declaration inside a `#[cfg(test)]` module plus one string in a test
assertion. The break is therefore theoretical for these trees, by
measurement rather than by assumption.

Naming the declaring paths has a shape consequence: provenance has to
survive as far as the check, so the duplicate test runs while items are
still grouped by the crate they came from, not inside the flat item walk
where every source has already been concatenated away.

An unresolved reference is treated differently on purpose. A duplicate
is always a mistake in code the author controls; a reference that
resolves nowhere may be a type the framework legitimately cannot reach,
so it warns rather than refuses, and that warning is CLI-only for the
same proc-macro reason.

The `cargo metadata` layer stays, narrowed to what ADR-0008 already
planned for it: locating a distributed extension crate, and now also
locating the declaration of a referenced type, rather than sweeping the
graph for annotations.

Rebuild tracking narrows with the scan. The `generate_idl!` macro
registers the files the layout assembly and the demand lookups actually
read, instead of every file in the graph, so a change to a file the
lookup's text filter skipped does not re-expand the macro. The lost
case is adding a referenced type to a file that never mentioned it,
which the next clean build picks up; the compiled-in IDL of
`lez_program` never registered dependency files at all.

A path dependency the consumer links but never stores can still
contribute a layout. This is accepted: linking it is the same deliberate
act the extension trust model already relies on for markers, and
narrowing further would need a signal the source does not carry.

## Considered alternatives

**Leave the scan graph-wide.** No output change and no work. Rejected
because it leaves a catalogue that anything in the tree can write to,
keeps 244 irrelevant crates in every parse, and contradicts what
`#[account_type]` is documented to mean.

**Keep scanning everything but refuse duplicate names.** Closes the
shadowing case without touching scope. Rejected as the primary fix: it
turns a silent wrong answer into a build failure caused by a crate the
program never uses, and leaves `AuthoritySlot` misfiled.

**Declare the IDL surface in extension metadata.** An extension states
which crates hold its types. Bounded and checkable, and it was the first
shape considered. Rejected because it is a field that must track
reality, and the day a field's type moves to a new crate and the
manifest is not updated, consumers get a thinner IDL with no error,
since unresolved references are tolerated. The connection is derivable,
so deriving it cannot drift.
