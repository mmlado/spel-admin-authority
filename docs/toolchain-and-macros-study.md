# LEE Toolchain Setup and `spel-framework-macros` Codebase Study

## LEE Toolchain

The library targets LEZ programs compiled with the RISC Zero zkVM. Working toolchain confirmed:

| Component | Version | Purpose |
| --- | --- | --- |
| `rzup` | 0.5.1 | RISC Zero toolchain manager |
| `cargo-risczero` | 3.0.5 | Cargo subcommand for building LEZ guest binaries |
| `cargo-expand` | latest | Inspecting macro output during development |

### Install

```bash
curl -L https://risczero.com/install | bash
rzup install
cargo install cargo-expand
```

### Verify

```bash
rzup --version            # rzup 0.5.1
cargo risczero --version  # cargo-risczero 3.0.5
cargo expand --version    # any recent
```

### Build modes

- **`RISC0_DEV_MODE=1`**: skips proof generation; runs guest code natively. Used for unit and integration tests.
- **`RISC0_SKIP_BUILD=1`**: skips guest ELF rebuild; used to keep `cargo clippy` fast in CI.
- Production builds (no env vars set) generate ZK proofs and take significantly longer.

```bash
RISC0_DEV_MODE=1 cargo test --workspace
RISC0_SKIP_BUILD=1 cargo clippy --workspace --all-targets -- -D warnings
```

## `spel-framework` codebase study

Two crates carry the integration work for admin-authority:

- [`spel-framework-macros`](https://github.com/mmlado/spel/blob/feat/admin_authority/spel-framework-macros/src/lib.rs), the proc-macro crate. Compile-time generation of dispatch, validators, and IDL.
- [`spel-framework-core::idl_gen`](https://github.com/mmlado/spel/blob/feat/admin_authority/spel-framework-core/src/idl_gen.rs), behind the `idl-gen` feature. Shared parsing + path-dep scanning consumed by both the proc-macros and the runtime IDL generator used by `spel-cli generate-idl`.

The framework hosts no admin-specific code. It provides a generic extension-discovery mechanism (described below) that admin-authority is the first consumer of.

### Public entry points

| Macro | Kind | Notes |
| --- | --- | --- |
| `#[lez_program]` | `proc_macro_attribute` | Orchestrator. Walks the module body, classifies items, emits the dispatch, validators, and `PROGRAM_IDL_JSON` const. Also drives extension discovery. |
| `#[instruction]` | `proc_macro_attribute` | Marker, pass-through. Consumed by `lez_program` expansion to identify instruction fns. |
| `#[account_type]` | `proc_macro_attribute` | Marker, pass-through. Consumed by the IDL generator. |
| `generate_idl!` | `proc_macro` (function-like) | Reads a source file from disk and emits IDL JSON at compile time of a host helper binary. |

### Extension discovery (the mechanism admin-authority hooks into)

Path-dep libraries declare themselves in their own `Cargo.toml`:

```toml
[package.metadata.spel]
extension_attr = "admin_authority"
instruction_attrs = ["require_admin"]
```

When a consumer's `#[lez_program]` module carries `#[admin_authority]`, the framework scans path-deps for matching metadata, parses the matched library's `src/lib.rs` for `#[instruction]` fns, and merges them into the consumer's pipeline with cross-crate dispatcher calls (`::admin_authority::admin_initialize(...)`).

The framework helpers live in `spel-framework-core::idl_gen`:

- `read_spel_extension_attr(crate_dir) -> Option<String>`
- `read_spel_instruction_attrs(crate_dir) -> Vec<String>`
- `discover_extension_instructions(manifest_dir, mod_attrs) -> Vec<(syn::ItemFn, syn::Path)>`
- `discover_extension_instruction_attrs(manifest_dir, mod_attrs) -> Vec<String>`
- `collect_instruction_fns(items) -> Vec<syn::ItemFn>`

Reuses path-dep walking machinery from [PR #180](https://github.com/logos-co/spel/pull/180) (`find_path_dep_dirs`, `collect_items_from_crate_dirs`).

### Internal seams the integration uses

| Function | Role | How extensions affect it |
| --- | --- | --- |
| `expand_lez_program()` | Top-level orchestrator. Parses module items, classifies them, generates the enum, match arms, handlers, validators, and IDL. | Calls `discover_extension_instructions`. Each discovered fn is parsed into `InstructionInfo` with `external_call_path = Some(::<crate>::<fn>)`. |
| `expand_generate_idl()` | Compile-time IDL generator that powers the `generate_idl!` proc-macro. | Same discovery loop, so IDL JSON emitted by host helper binaries matches the consumer's compiled dispatcher. |
| `parse_instruction()` | Classifies fn params into `accounts` and `args`, captures attribute metadata into `InstructionInfo`. | Unchanged shape. Called for both user-defined and discovered extension fns. |
| `generate_handler_fns()` | Emits handler functions verbatim minus macro markers. | Two changes: skips fns with `external_call_path.is_some()` (the body lives in the library); strips attrs listed in the collected `instruction_attrs` to prevent re-expansion of library-owned gate macros. |
| `generate_match_arms()` | Emits dispatcher arms. | Uses `external_call_path` when present; falls back to bare-name local call otherwise. |
| `InstructionInfo` struct | Per-instruction parsed model. | Gained an `external_call_path: Option<syn::Path>` field. |

### Runtime building blocks (consumed, not modified)

| Item | Location | Use |
| --- | --- | --- |
| `AccountId`, `AccountWithMetadata` | `spel-framework-core` | Account identity and runtime metadata (signature flag). |
| `ProgramId` | `spel-framework-core` | Program identity, used in PDA derivation. |
| `compute_pda`, `ToSeed` | `spel-framework-core::pda` | PDA address derivation. |
| `SpelError::Unauthorized` | `spel-framework-core::error` | Library `AdminError` maps onto this at the SPEL boundary. |
| `is_authorized` field | `AccountWithMetadata` | LEZ-verified signature marker; the foundation of the admin signer check. |

### Pattern this integration follows

Marker attributes (`#[admin_authority]`, `#[require_admin]`) are pass-through proc-macros defined in `admin-authority-macros`. The framework detects their presence by attribute name only, never invoking the library's macros for discovery. The work happens inside `expand_lez_program()` (and the parallel IDL paths), which scan the consumer's source tokens for those markers and act on them.

Library authors own:

- The marker attribute's expansion (typically pass-through).
- Any per-instruction gate attributes (e.g. `#[require_admin]`) and their shape validation.
- The `#[instruction]` fn definitions and method bodies.
- The discovery metadata in their `Cargo.toml`.

The framework owns:

- The path-dep scan loop driven by metadata.
- Dispatcher and validator codegen, agnostic of which extension produced the instruction.
- Stripping `instruction_attrs` from emitted handlers so library-owned gate macros don't re-expand.

This split lets new extensions (e.g. RFP-002 freeze-authority) ship without any framework PR.
