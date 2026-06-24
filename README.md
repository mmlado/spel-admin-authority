# SPEL Admin Authority

Single-admin authority primitive for LEZ programs. Provides a standardised way to gate privileged instructions behind a transferable, renounceable admin, integrated as two SPEL macros so consumers add it with one or two annotations.

## What it does

A program adds `#[admin_authority]` at the module level and `#[require_admin]` on each instruction it wants gated. The library ships the three management instructions (`admin_initialize`, `admin_transfer`, `admin_renounce`), and the framework discovers them at compile time via metadata declared in the library's `Cargo.toml`. `#[require_admin]` emits a runtime check on each gated instruction confirming the caller is the current admin.

```rust
use spel_framework::prelude::*;
use admin_authority::{admin_authority, require_admin};

#[lez_program]
#[admin_authority]
mod my_program {
    #[instruction]
    #[require_admin]
    pub fn update_value(
        #[account(pda = literal("admin_config"))] admin_config: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        // handler body. The admin check has already run.
    }
}
```

Adding `#[admin_authority]` to the module exposes three new instructions in the IDL:

- `admin_initialize` creates the Config PDA and sets the first admin.
- `admin_transfer` replaces the current admin with a new one.
- `admin_renounce` zeros the admin permanently. Terminal.

Adding `#[require_admin]` to any instruction inserts a check that decodes the admin config and asserts the caller is the current admin before the handler body runs.

## Layout

| Crate | Purpose |
|---|---|
| [`admin-authority`](admin-authority/) | Runtime library: `AdminConfig`, `AdminCandidate`, `AdminError`, the auth methods, and the three management instruction fns. Declares the discovery metadata. |
| [`admin-authority-macros`](admin-authority-macros/) | Proc-macro sub-crate: `#[admin_authority]` (marker), `#[require_admin]` (shape validator + body injector). Re-exported through `admin-authority`. |
| [`admin-authority-sample`](admin-authority-sample/) | Reference SPEL program that uses both macros end to end. |

## Architecture

Framework knows nothing specific about admin-authority. Generic extension scanner in `spel-framework-core::idl_gen` walks path-deps looking for `[package.metadata.spel]` declarations:

```toml
# admin-authority/Cargo.toml
[package.metadata.spel]
extension_attr = "admin_authority"
instruction_attrs = ["require_admin"]
```

When the consumer's `#[lez_program]` module carries `#[admin_authority]`, the scanner reads admin-authority's `src/lib.rs` for `#[instruction]`-annotated fns and merges them into the consumer's dispatcher + IDL with cross-crate call paths (`::admin_authority::admin_initialize(...)`). Same mechanism powers any future extension (e.g. `freeze-authority`); no framework PR needed per library.

## Adding as a dependency

```toml
[dependencies]
admin-authority = { git = "https://github.com/mmlado/spel-admin-authority" }
spel-framework  = { git = "https://github.com/mmlado/spel", branch = "feat/admin_authority" }
```

`admin-authority-macros` is pulled in transitively via `admin-authority`, no need to declare it directly.

(The `spel-framework` URL points at the fork branch carrying the generic extension scanner. Will move to `logos-co/spel` once the upstream PR merges.)

## Integration steps

1. **Annotate the module** with `#[admin_authority]` after `#[lez_program]`. The three admin instructions appear in the IDL automatically.
2. **Call `admin_initialize`** as part of deployment, bundling it with the deploy if possible. Anything between deployment and the first `admin_initialize` is the [initialization window](docs/authority-lifecycle.md#initialization-window-risk); whoever calls first becomes admin.
3. **Gate instructions** by adding `#[require_admin]` and declaring an `#[account(pda = literal("admin_config"))]` param and an `#[account(signer)]` param. The macro's strict-mode check refuses to compile if either is missing.
4. **Transfer or renounce** via the injected `admin_transfer` and `admin_renounce` instructions. Transfer takes an `AdminCandidate` (signer or PDA) paired with the corresponding `new_admin_account`.

The [authority lifecycle document](docs/authority-lifecycle.md) covers the state machine, validation rules at each transition, and the program-as-admin path through CPI.

## Security notes

- **Initialization window.** Call `admin_initialize` immediately after deployment, ideally bundled with it. Until that call lands, anyone can submit it and become admin.
- **Renounce is terminal.** `admin_renounce` writes `AccountId::default()` and that is the end. No recovery path by design.
- **PDA admins via CPI.** A program-owned PDA can be the admin. The owning program calls the gated instruction via a chained_call and declares its admin PDA in `caller-pda-seeds`; LEZ propagates `is_authorized` to the callee. See the lifecycle doc.
- **Transfer history.** Not recorded on-chain. The current admin is always readable from the Config PDA; historical transfers require an off-chain indexer.

## Documentation

- [`docs/authority-lifecycle.md`](docs/authority-lifecycle.md): state machine, transitions, validation rules.
- [`docs/adr/`](docs/adr/): architectural decision records (PDA seed, macro ordering, strict-by-default).
- [`CONTEXT.md`](CONTEXT.md): domain language used throughout the project.

## Development

```bash
cargo check --workspace
RISC0_DEV_MODE=1 cargo test --workspace
cargo expand -p admin-authority-sample
```

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE2) at the consumer's option.
