# LEE Toolchain Setup and `spel-framework-macros` Codebase Study

## LEE Toolchain

The library targets LEZ programs compiled with the RISC Zero zkVM. Working toolchain confirmed:

| Component | Version | Purpose |
|---|---|---|
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

## `spel-framework-macros` Codebase Study

The SPEL proc-macro crate is a single file ([`spel-framework-macros/src/lib.rs`](https://github.com/logos-co/spel/blob/main/spel-framework-macros/src/lib.rs), 2,476 lines as of the version studied). Adding `#[admin_authority]` and `#[require_admin]` extends three existing seams without rearchitecting anything.

### Public entry points

| Macro | Kind | Notes |
|---|---|---|
| `#[lez_program]` | `proc_macro_attribute` | The orchestrator. Walks the module body, classifies items, emits the dispatch and validators. |
| `#[instruction]` | `proc_macro_attribute` | Marker, pass-through. Consumed by `lez_program` expansion. |
| `#[account_type]` | `proc_macro_attribute` | Marker, pass-through. Consumed by the IDL generator. |
| `generate_idl!` | `proc_macro` (function-like) | Reads a source file from disk and emits IDL JSON. |

### Internal seams used by this integration

| Function | Role | What this integration adds |
|---|---|---|
| `expand_lez_program()` | Top-level orchestrator. Walks module items, splits them into `instructions` and `other_items`, then generates the enum, match arms, handlers, validators, and IDL. | Detects `#[admin_authority]` on the module and injects three synthetic `ItemFn` nodes (the admin instructions) into the same parse pipeline. |
| `parse_instruction()` | Classifies fn params into `accounts` and `args`, captures attribute metadata into `InstructionInfo`. | Detects `#[require_admin]`, sets a `require_admin: bool` field, and performs the strict-mode shape check (presence of `admin_config` PDA and signer params). |
| `generate_handler_fns()` | Emits the handler functions verbatim minus macro markers. | Strips the `#[require_admin]` attribute so it doesn't re-fire as a standalone proc-macro after expansion. |
| `generate_validation()` (M2 codegen) | Emits per-instruction `__validate_*` functions with signer / init / PDA checks. | Will read `InstructionInfo.require_admin` to emit the `AdminConfig::decode` + `assert_admin` prologue (M2 scope). |
| `InstructionInfo` struct | Per-instruction parsed model. | Gains a `require_admin: bool` field. |

### Shared module (cross-crate)

`spel-framework-core::admin_authority` holds the helpers that are needed by both the proc-macros and the runtime IDL generator (`spel-framework-core::idl_gen`). The module is gated behind the `idl-gen` feature so the runtime crate only pulls `syn` when it actually needs the IDL pipeline.

The module exports:

- `has_admin_authority_attr(attrs: &[Attribute]) -> bool`
- `has_require_admin_attr(attrs: &[Attribute]) -> bool`
- `admin_instruction_fns() -> Vec<ItemFn>` — returns the three synthetic instruction templates (`admin_initialize`, `admin_transfer`, `admin_renounce`).

This split keeps the macro crate (`spel-framework-macros`) and the IDL generator in sync: both call the same source of truth when looking for admin authority annotations and when injecting admin instructions.

### Runtime building blocks (consumed, not modified)

| Item | Location | Use |
|---|---|---|
| `AccountId`, `AccountWithMetadata` | `spel-framework-core` | Account identity and runtime metadata (signature flag). |
| `ProgramId` | `spel-framework-core` | Program identity, used in PDA derivation. |
| `compute_pda`, `ToSeed` | `spel-framework-core::pda` | PDA address derivation. |
| `SpelError::Unauthorized` | `spel-framework-core::error` | Library `AdminError` maps onto this at the SPEL boundary. |
| `is_authorized` field | `AccountWithMetadata` | LEZ-verified signature marker; the foundation of the admin signer check. |

### Pattern this integration follows

Marker attributes (`#[admin_authority]`, `#[require_admin]`) are pass-through proc-macros that emit `compile_error!` if they ever run as standalone macros. The actual work happens inside `expand_lez_program` and `parse_instruction`, which scan the consumer's source tokens for those markers and act on them. This mirrors how `#[instruction]` already works, so the integration introduces zero new patterns.
