# SPEL Admin Authority

Single-admin authority primitive for LEZ programs. Provides a standardised way to gate privileged instructions behind a transferable, renounceable admin, integrated as two SPEL macros so consumers add it with one or two annotations.

## What it does

A program adds `#[admin_authority]` at the module level and `#[require_admin]` on each instruction it wants gated. The library ships the three management instructions (`admin_initialize`, `admin_transfer`, `admin_renounce`), and the framework discovers them at compile time via metadata declared in the library's `Cargo.toml`.

Status at this milestone (M2.5): the library is working, gate params are injected by the framework when a gated instruction does not declare them, and the admin slot can live either in its own Config PDA (dedicated mode, the default) or inside one of the consumer's own accounts at a byte offset (embedded mode, see below). All three reference samples pass behavioural tests.

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
        #[account(signer)] caller: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut config: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        // handler body. The admin check runs before this.
    }
}
```

The gate reads two accounts: `admin_config`, the Config PDA, and `caller`, the signer. Declaring them is optional: a gated instruction that omits either gets it synthesized by the framework from the library's inject metadata, PDA-verified and part of the IDL. With different param names, pass them to the gate: `#[require_admin(admin_config = my_cfg, caller = owner)]`.

Adding `#[admin_authority]` to the module exposes three new instructions in the IDL:

- `admin_initialize` creates the Config PDA and installs the caller as the first admin (self-election, see [ADR-0005](docs/adr/0005-self-election-via-caller.md)).
- `admin_transfer` replaces the current admin with a new one.
- `admin_renounce` zeros the admin permanently. Terminal.

Adding `#[require_admin]` to an instruction marks it admin-gated: it inserts a check that decodes the admin config and asserts the caller is the current admin before the handler body runs.

## Embedded mode

The admin slot can live inside one of the consumer's own accounts instead of a dedicated Config PDA. Declared program-wide on the marker, role kwarg plus byte offset:

```rust
#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ProgConfig {
    pub value: u64,            // bytes 0..8
    pub padding: [u8; 24],     // bytes 8..32
    pub admin: AdminConfig,    // bytes 32..64, the embedded slot
}

#[lez_program]
#[admin_authority(admin_config = config, offset = 32)]
mod my_program {
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("prog_config"))] mut config: AccountWithMetadata,
        #[account(signer)] signer: AccountWithMetadata,
    ) -> SpelResult {
        ProgConfig { value: 0, padding: [0; 24], admin: AdminConfig::default() }
            .write_to(&mut config)?;
        AdminConfig::bootstrap_at(&mut config, 32, AdminCandidate::Signer, &signer)?;
        // ...
    }
}
```

What changes versus dedicated mode:

- **No `admin_initialize`.** The slot is born initialized: the consumer's own account-creating instruction writes its struct, then `bootstrap_at` splices the admin in. An account created without the bootstrap is born renounced, permanently. There is no init front-running window in embedded mode.
- **Everything retargets.** Gates read the slot at the declared offset from the embedding account, `admin_transfer` and `admin_renounce` operate on it (writes splice only the 32-byte window, neighboring consumer fields survive), and the IDL shows the embedding account everywhere the dedicated PDA used to appear.
- **The offset is never in a transaction.** It is compiled into the program as a literal at every call site; the IDL carries no offset argument. Changing it means different bytecode, which on LEZ is a different program.
- **The marker is the only writer of location kwargs.** Writing `admin_config = ...` or `offset = ...` on a gate by hand is a compile error in embedded mode; the `caller` kwarg stays available.
- **Layout obligations.** The embedded `AdminConfig` field must sit at the declared offset with only fixed-size fields before it, and the embedding account must be declared under the marker's name in every instruction that declares it.

Embedded mode removes one account from every gated transaction. Design record: [ADR-0007](docs/adr/0007-embedded-account-support.md). Dry-run walkthrough: `scripts/dry-run-embedded.sh`, expected output in [`docs/dry-run-embedded-output.txt`](docs/dry-run-embedded-output.txt).

## Layout

| Crate | Purpose |
|---|---|
| [`admin-authority`](admin-authority/) | Runtime library: `AdminConfig`, `AdminCandidate`, `AdminError`, the auth methods, and the three management instruction fns. Declares the discovery metadata. |
| [`admin-authority-macros`](admin-authority-macros/) | Proc-macro sub-crate: `#[admin_authority]` (marker), `#[require_admin]` (injects the runtime admin check at the top of the handler body). Re-exported through `admin-authority`. |
| [`admin-authority-sample`](admin-authority-sample/) | Reference SPEL program using both macros end to end, with declared gate params. |
| [`admin-authority-sample-manual`](admin-authority-sample-manual/) | Second reference program showing the manual path: no `#[admin_authority]` marker, self-elect initialize inside the consumer's own handler, hand-written transfer and renounce, fully declared gate params. |
| [`admin-authority-sample-embedded`](admin-authority-sample-embedded/) | Third reference program: the admin slot embedded inside the consumer's own `prog_config` account at byte offset 32, bootstrapped by the consumer's initialize, management instructions retargeted by the framework. |

## Architecture

Framework knows nothing specific about admin-authority. A generic extension scanner in `spel-framework-core` walks the consumer's direct dependencies (path, git, or registry) looking for `[package.metadata.spel]` declarations:

```toml
# admin-authority/Cargo.toml
[package.metadata.spel]
extension_attr = "admin_authority"
```

When the consumer's `#[lez_program]` module carries `#[admin_authority]`, the scanner reads admin-authority's `src/lib.rs` for `#[instruction]`-annotated fns and merges them into the consumer's dispatcher and IDL with cross-crate call paths (`::admin_authority::admin_initialize(...)`).

The `#[require_admin]` gate check is an ordinary proc-macro that re-expands on the emitted handler, which is how it injects its runtime check ([ADR-0004](docs/adr/0004-require-admin-injection-contract.md)). The gate's account params come from the library's inject metadata when a gated instruction does not declare them ([ADR-0006](docs/adr/0006-param-injection-and-relaxed-mode.md)); in embedded mode the framework additionally rewrites the `admin_config` role to the consumer's embedding account and stamps every gate with the location kwargs ([ADR-0007](docs/adr/0007-embedded-account-support.md)).

The same mechanism powers any future extension such as `freeze-authority`, with no framework PR needed per library.

## Adding as a dependency

The framework discovers extensions among the consumer's direct dependencies, whether they come by path, git, or registry. `admin-authority` must be a direct dependency; a transitive one is never discovered, by design.

```toml
[dependencies]
admin-authority = { git = "https://github.com/mmlado/spel-admin-authority", branch = "m2_5" }
spel-framework  = { git = "https://github.com/mmlado/spel", branch = "feat/admin_authority_m2_5" }
```

A local checkout referenced by `path` works the same way. `admin-authority-macros` is pulled in transitively via `admin-authority`, no need to declare it directly. The `spel-framework` branch must match the one this repo's Cargo.toml pins, `feat/admin_authority_m2_5` at this milestone. It moves to `logos-co/spel` once the upstream PR merges.

## Integration steps

1. **Annotate the module** with `#[admin_authority]` after `#[lez_program]`. The three admin instructions appear in the IDL automatically.
2. **Call `admin_initialize`** immediately after deployment; the caller becomes admin. Bundling with the deploy is not possible on LEZ today (deployment transactions carry no instructions). Anything between deployment and the first `admin_initialize` is the [initialization window](docs/authority-lifecycle.md#initialization-window-risk); whoever calls first becomes admin. Want a different admin? Initialize, then `admin_transfer`.
3. **Gate instructions** by adding `#[require_admin]`. Declaring the `admin_config` and `caller` params is optional, missing ones are injected. Custom names go through the gate's args: `#[require_admin(admin_config = my_cfg, caller = owner)]`.
4. **Transfer or renounce** via the injected `admin_transfer` and `admin_renounce` instructions. Transfer takes an `AdminCandidate` (signer or PDA) paired with the corresponding `new_admin_account`.

The [authority lifecycle document](docs/authority-lifecycle.md) covers the state machine, validation rules at each transition, and the program-as-admin path through CPI.

## Security notes

- **Initialization window.** Call `admin_initialize` immediately after deployment. Until that call lands, anyone can submit it and become admin. Bundling with the deployment is not possible on LEZ today (deployment transactions carry no instructions), so the window is structural.
- **Renounce is terminal.** `admin_renounce` writes `AccountId::default()` and that is the end. No recovery path by design.
- **PDA admins via CPI.** A program-owned PDA can be the admin. The owning program calls the gated instruction via a chained_call and declares its admin PDA in `caller-pda-seeds`; LEZ propagates `is_authorized` to the callee. See the lifecycle doc.
- **Transfer history.** Not recorded on-chain. The current admin is always readable from the Config PDA; historical transfers require an off-chain indexer.

## Documentation

- [`docs/authority-lifecycle.md`](docs/authority-lifecycle.md): state machine, transitions, validation rules.
- [`docs/adr/`](docs/adr/): architectural decision records (PDA seed, macro placement, self-election, gate check injection, param injection, embedded account support).
- [`CONTEXT.md`](CONTEXT.md): domain language used throughout the project.

## Development

```bash
cargo check --workspace
RISC0_DEV_MODE=1 cargo test --workspace
cargo expand -p admin-authority-sample
```

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE2) at the consumer's option.
