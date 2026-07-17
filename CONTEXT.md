# Admin Authority

A SPEL library that brings the Solidity Ownable pattern to LEZ programs. One admin per program deployment, and admin-gated instructions reject all other callers.

## Language

**Program**:
A stateless ELF binary deployed to LEZ. Identified by its image_id (hash of the binary). Each unique binary has a unique program_id and an isolated PDA namespace.
_Avoid_: contract, smart contract

**Admin**:
The single `AccountId` stored in the Config PDA that is authorized to call admin-gated instructions. Set at `admin_initialize` by self-election: the signing caller becomes the Admin, and no candidate argument exists at initialize, because LEZ rejects a transaction whose account list contains the same account id twice, so a caller could never also pass itself as candidate evidence. An external keyholder or PDA becomes Admin via a subsequent `admin_transfer`. Transferable or permanently renounced.
_Avoid_: owner (Solidity term), authority (ambiguous with LEZ's `is_authorized`); describing initialize as taking an `AdminCandidate` (that is transfer-time only)

**Config PDA**:
The on-chain account that stores the `AdminConfig` state. Derived from `(program_id, "admin_config")`. Created once via `admin_initialize`, and cannot be reinitialized.
_Avoid_: admin account, state account, "config" (too generic, since consumers may already claim that seed for unrelated program state)

**Admin-gated instruction**:
A SPEL instruction annotated with `#[require_admin]`. From M2, the gate rejects the transaction if the signer is not the current Admin, before the handler body runs. At M1 the macro validates that the gate params are declared.
_Avoid_: protected instruction, restricted instruction

**Renounce**:
Permanent, irreversible removal of admin authority. Zeros `admin` to `AccountId::default()` in the Config PDA. There is no recovery path and no reinit possible (the PDA still exists, and `#[account(init)]` rejects reuse).
_Avoid_: revoke (ambiguous; could imply reversibility), burn

**AdminError**:
Custom error enum in the `admin-authority` library crate. Library methods return `AdminError`, and instruction handlers map it to `SpelError::Unauthorized` at the SPEL boundary (method bodies are stubs at M1, the returns land in M2). This keeps the library independent of SPEL's error types.
_Avoid_: returning `SpelError` directly from library methods

**AdminCandidate**:
Transfer-time argument describing the intended new admin. `Signer` carries no data; validation checks `new_admin_account.is_authorized` (co-signed the tx). `Pda { program_id, seed }` is validated by deriving the address via `AccountId::for_public_pda` and confirming the PDA is initialized. Distinct from `AdminConfig.admin`, which stores only the resolved `AccountId`. Always paired with a `new_admin_account: AccountWithMetadata` parameter; `AdminCandidate` is the claim, `AccountWithMetadata` is the chain-state evidence. One without the other provides no security guarantee.
_Avoid_: using a bare `AccountId` arg for transfer (cannot validate key ownership or PDA existence)

**Transfer history**:
Not recorded on-chain in this release. The current admin is always readable from the Config PDA; historical transfers require an off-chain indexer. Future improvement: once `lez-events` (LP-0012) lands in an official LEZ release, the library will emit typed events (`AdminInitialized`, `AdminTransferred`, `AdminRenounced`) from its methods. No extra accounts, and queryable via `getTransactionReceipt`.

**Initialization window**:
The period between program deployment and the first call to `admin_initialize`. During this window the Config PDA does not exist and any caller can front-run and become admin. Deployers must call `admin_initialize` immediately after deployment.
_Avoid_: "setup phase" (too vague)

**Strict mode**:
Default behavior of `#[require_admin]`. Emits `compile_error!` if the annotated instruction does not declare an `admin_config` param and a `caller` (or `signer`) param, matched by name. Recommended for all production programs.
_Avoid_: calling this "explicit mode"

**Param injection (M2)**:
The planned relaxation: the framework synthesizes the gate params from inject metadata the library will declare in its Cargo.toml, so consumers stop typing them on every gated instruction. Lands in M2 and supersedes the earlier env-var relaxed-mode design from ADR-0003.
_Avoid_: the `SPEL_ADMIN_AUTHORITY_RELAXED` env var (superseded before it shipped)

**Injected instructions**:
The three instructions added to a consumer's module by `#[admin_authority]`: `admin_initialize`, `admin_transfer`, `admin_renounce`. They appear in the IDL and are callable via SPEL CLI. Source lives as real `#[instruction] fn` definitions in `admin-authority/src/lib.rs`; the framework discovers them via a path-dep scan triggered by the `#[admin_authority]` marker and emits cross-crate dispatch calls (`::admin_authority::admin_initialize(...)`) into the consumer's binary. They never exist as copy-pasted source in the consumer module.
_Avoid_: "generated functions" (implies they exist as source in the consumer); "synthesized templates" (was true pre-pivot, no longer accurate); "admin_init" or "init_admin" (wrong prefix/suffix convention)

**Extension attr (framework discovery mechanism)**:
The trigger by which the SPEL framework discovers a third-party library's instructions. Each extension library declares its marker attribute name in `[package.metadata.spel.extension_attr]` in its `Cargo.toml`. When a consumer's `#[lez_program]` module carries that attribute, the framework scans the library's `src/lib.rs` for `#[instruction]`-annotated fns and includes them in the consumer's dispatcher + IDL. `admin-authority` declares `extension_attr = "admin_authority"`, so `#[admin_authority]` on a consumer mod triggers the scan. Framework is library-agnostic; the same mechanism powers any future extension (e.g. `freeze-authority`).
_Avoid_: "plugin", "hook" (imply runtime registration; this is compile-time discovery)

**admin-authority-macros (sub-crate)**:
Proc-macro crate that ships alongside the `admin-authority` library. Provides `#[admin_authority]` (marker, pass-through), `#[require_admin]` (param-name validator at M1; the runtime check lands in M2), and an internal `#[instruction]` shim that strips `#[account(...)]` helper attrs so the library's own source compiles in isolation. Required because attribute macros must live in a `proc-macro = true` crate, separate from the runtime library.
_Avoid_: "macros crate" (too generic); merging into `admin-authority` (cannot, proc-macro crates can't export non-macro items)

## Relationships

- A **Program** has exactly one **Config PDA**.
- A **Config PDA** holds exactly one **Admin** at any time, or is renounced.
- An **Admin-gated instruction** reads the **Config PDA** to verify the caller is the current **Admin**.
- **Renounce** transitions the **Config PDA** to a terminal state. No further admin-gated instructions can succeed.

**Sample program**:
One of two reference implementations planned for the library. `admin-authority-sample` shows the macro-driven path (`#[admin_authority]` plus a separate `admin_initialize`) and ships at M1. `admin-authority-sample-manual` shows the manual-init path (`AdminConfig::initialize` called inside the consumer's own `initialize`) and ships in M2. Both contain `update_value` gated by `#[require_admin]`, `admin_transfer`, and `admin_renounce`, with integration tests.
_Avoid_: demo, example (implies optional; the samples are hard RFP deliverables)

## Example dialogue

> **Dev:** "Can two different users each be admin of the same program?"
> **Domain expert:** "No. There's one admin per program. If you want separate admin domains, each consumer deploys their own ELF. They get their own Config PDA and their own admin."

> **Dev:** "What happens if the admin loses their key after renouncing?"
> **Domain expert:** "Nothing. Renounce is terminal. The Config PDA is frozen. Admin-gated instructions are permanently blocked. Design the program so renounce is only callable when that's the intended outcome."
