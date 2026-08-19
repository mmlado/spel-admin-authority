# Extensions self-emit; the Cargo.toml metadata scanner is removed

Status: Proposed. Post-contract design exploration, not part of any milestone deliverable. The extension-scanner architecture is the accepted contract baseline. Not yet reviewed by the maintainers, no action requested. Date: 2026-07-28.

Extensions add their instructions by **self-emission**: the extension's attribute macro (outer to `#[lez_program]`) parses the consumer module at compile time and pushes its instruction functions in as real `Item::Fn` nodes, injects gate params, and rewrites `execute(...)` — so `#[lez_program]` sees a complete module with no discovery step. The framework's `[package.metadata.spel]` scanner (`discover_extension`, the `find_path_dep_dirs`-driven extension discovery, the Cargo.toml metadata readers) is deleted. The single source of truth for an extension is its `spel_extension!` declaration. See [extension-model.md](../extension-model.md).

## Why

The scanner located and parsed extension crates **inside the compiler, on every build**. Even after unifying it to one `find_path_dep_dirs` call, the `cargo metadata` subprocess it needs for git/crates.io deps costs ~200–300ms per compile (subprocess spawn + graph resolve), versus ~5ms of actual work. Self-emission removes cross-crate discovery from the build path entirely: the extension macro emits its instructions directly, and cargo resolves the extension dependency the normal way, identically for path/git/crates.io. Build-time `cargo metadata` drops to zero.

## Considered Options

**1. Self-emission, scanner removed (chosen).** Build path is `cargo metadata`-free and delivery-agnostic. IDL (`spel-cli generate-idl`, which cannot run macros) reads the same `spel_extension!` declaration by parsing its input tokens from source and replaying the transform functions. `cargo metadata` survives only there — once, on demand, only to locate a distributed extension crate.

**2. Keep the (unified) scanner.** Already working and tested. But a subprocess's fixed cost can't be optimized away, only not-paid; unifying calls shrank it but couldn't remove it from every build. And it keeps extension metadata in Cargo.toml, which the IDL side would then also have to read — two definitions of the same thing.

## Consequences

- **`[package.metadata.spel]` is gone.** Extensions declare everything in `spel_extension!`. No Cargo.toml metadata blocks.
- **Single source of truth.** The `spel_extension!` declaration is read by the extension's own macro (build) and by `spel-cli` (IDL). The shared grammar parser and transform functions live in `spel-framework-core`; the `spel_extension!` macro lives in `spel-framework-macros`.
- **`spel_extension!` is a `#[proc_macro]` that generates the extension's `#[attr]` attribute macro** (not a `macro_rules!`), so its grammar is parsed by one `syn` function in core rather than duplicated as macro patterns plus a separate CLI parser.
- **Attribute order is load-bearing** — the extension attribute must be outer to `#[lez_program]` (already recorded in ADR-0002 for the scanner era; still true, now because the macro must run first to emit). Wrong-order misuse currently fails unclearly; a clear diagnostic is an open follow-up (see LEZ_PROGRAM_ATTR_DROP_REPORT.md).
- **IDL for a distributed extension is not `cargo metadata`-free** — the CLI still needs it once to locate a git/crates.io extension crate. Accepted: it runs in an occasional CLI invocation, not in every compile.
