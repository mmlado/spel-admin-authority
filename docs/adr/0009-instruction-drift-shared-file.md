# Instruction bodies live in a shared file, compiled in-crate to catch drift

Status: Proposed. Post-contract design exploration, not part of any milestone deliverable. The extension-scanner architecture is the accepted contract baseline. Not yet reviewed by the maintainers, no action requested. Date: 2026-07-28.

An extension's instruction definitions live in one `instructions.rs` file. The **library crate `include!`s it**, so the instruction functions compile as real code against the lib's own helpers (`AdminConfig::bootstrap`, etc.) during the lib's normal `cargo build` — any signature drift is a compile error there. The **`-macros` crate `include_str!`s the same file** at its own build, baking the source as tokens the attribute macro later emits into a consumer. `spel-cli` reads the same file for IDL. One definition, three readers.

## Why

With self-emission, the risk is that an instruction body (which calls into the lib) drifts from the lib's actual API. If the bodies existed only as tokens inside `spel_extension!`, they'd never be type-checked until a *consumer* expanded the macro — so an extension author could ship a broken extension and only a downstream build would fail. We require the guarantee to hold in the extension's **own** build, not a sample/CI step an author might omit ("we can't be sure what others will do").

The macro cannot read the lib's source at consumer-expansion time to emit it, because its `CARGO_MANIFEST_DIR` is then the *consumer's* — locating the extension's own source would need `cargo metadata`, the build-time cost being deleted. So the instructions must be captured at the extension's own build, where paths are known: `include!` (lib, compiles them) + `include_str!` (`-macros`, bakes them).

## Considered Options

**1. Shared `instructions.rs`, `include!` + `include_str!` (chosen).** The whole emitted instruction (signature + body) compiles during the lib build, so drift is caught in-crate, by the author, before shipping. Reuses m3's "instructions are real source in the lib" (which already compiled them via the `#[instruction]` shim) — but the `-macros` crate bakes that file for self-emission instead of the framework scanning it.

**2. Delegation.** The lib exposes real functions; the emitted `#[instruction]` is a thin wrapper calling them. Simpler, but the wrapper-to-lib-fn signature match is still only checked at consumer build — narrows the drift surface without closing it in-crate. Rejected for not meeting the "guarantee in the author's own build" bar.

## Consequences

- The `spel_extension!` declaration **splits**: gating metadata (name, wrapper, inject accounts) stays in the block; the instruction bodies live in the shared `instructions.rs` the block references.
- The extension crate layout is load-bearing: `instructions.rs` must be reachable by both the lib (`include!`) and `-macros` (`include_str!`) at their build times — a fixed relative path within the extension workspace.
- `#[account(...)]` annotation correctness is still only validated by `#[lez_program]` at consumer build; the in-crate compile catches **body-vs-lib** drift (the risky part), not annotation shape.
