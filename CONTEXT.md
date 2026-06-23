# Admin Authority

A SPEL library that brings the Solidity Ownable pattern to LEZ programs. One admin per program deployment, and admin-gated instructions reject all other callers.

## Language

**Program**:
A stateless ELF binary deployed to LEZ. Identified by its image_id (hash of the binary). Each unique binary has a unique program_id and an isolated PDA namespace.
_Avoid_: contract, smart contract

**Admin**:
The single `AccountId` stored in the Config PDA that is authorized to call admin-gated instructions. Set at `admin_initialize` via a mandatory `new_admin: AdminCandidate` paired with `new_admin_account: AccountWithMetadata`. To self-elect, the caller passes `AdminCandidate::Signer` with their own account as `new_admin_account`. Transferable or permanently renounced.
_Avoid_: owner (Solidity term), authority (ambiguous with LEZ's `is_authorized`)

**Config PDA**:
The on-chain account that stores the `AdminConfig` state. Derived from `(program_id, "admin_config")`. Created once via `admin_initialize`, and cannot be reinitialized.
_Avoid_: admin account, state account, "config" (too generic, since consumers may already claim that seed for unrelated program state)

**Admin-gated instruction**:
A SPEL instruction annotated with `#[require_admin]`. The generated validator rejects the transaction if the signer is not the current Admin, before the handler body runs.
_Avoid_: protected instruction, restricted instruction

**Renounce**:
Permanent, irreversible removal of admin authority. Zeros `admin_authority` to `AccountId::default()` in the Config PDA. There is no recovery path and no reinit possible (the PDA still exists, and `#[account(init)]` rejects reuse).
_Avoid_: revoke (ambiguous; could imply reversibility), burn

**AdminError**:
Custom error enum in the `admin-authority` library crate. Library methods return `AdminError`, and instruction handlers map it to `SpelError::Unauthorized` at the SPEL boundary. This keeps the library independent of SPEL's error types.
_Avoid_: returning `SpelError` directly from library methods

**AdminCandidate**:
Transfer-time argument describing the intended new admin. `Signer` carries no data; validation checks `new_admin_account.is_authorized` (co-signed the tx). `Pda { program_id, seed }` is validated by deriving the address via `AccountId::for_public_pda` and confirming the PDA is initialized. Distinct from `AdminConfig.admin_authority`, which stores only the resolved `AccountId`. Always paired with a `new_admin_account: AccountWithMetadata` parameter; `AdminCandidate` is the claim, `AccountWithMetadata` is the chain-state evidence. One without the other provides no security guarantee.
_Avoid_: using a bare `AccountId` arg for transfer (cannot validate key ownership or PDA existence)

**Transfer history**:
Not recorded on-chain in this release. The current admin is always readable from the Config PDA; historical transfers require an off-chain indexer. Future improvement: once `lez-events` (LP-0012) lands in an official LEZ release, the library will emit typed events (`AdminInitialized`, `AdminTransferred`, `AdminRenounced`) from its methods. No extra accounts, and queryable via `getTransactionReceipt`.

**Initialization window**:
The period between program deployment and the first call to `admin_initialize`. During this window the Config PDA does not exist and any caller can front-run and become admin. Deployers must call `admin_initialize` immediately after deployment.
_Avoid_: "setup phase" (too vague)

**Strict mode**:
Default behavior of `#[require_admin]`. Emits `compile_error!` if the annotated instruction does not explicitly declare an `admin_config` PDA param and a signer param. Recommended for all production programs.
_Avoid_: calling this "explicit mode"

**Relaxed mode**:
Opt-in build-time behavior enabled by `SPEL_ADMIN_AUTHORITY_RELAXED=1`. The macro auto-injects `__admin_config` and `__admin_signer` accounts if absent. For prototyping only.
_Avoid_: treating relaxed as the default

**Injected instructions**:
The three instructions synthesized into a consumer's module by `#[admin_authority]`: `admin_initialize`, `admin_transfer`, `admin_renounce`. They appear in the IDL and are callable via SPEL CLI. Source lives as `quote!` templates inside `expand_lez_program()`, not in consumer source files.
_Avoid_: "generated functions" (implies they exist as source); "admin_init" or "init_admin" (wrong prefix/suffix convention)

## Relationships

- A **Program** has exactly one **Config PDA**.
- A **Config PDA** holds exactly one **Admin** at any time, or is renounced.
- An **Admin-gated instruction** reads the **Config PDA** to verify the caller is the current **Admin**.
- **Renounce** transitions the **Config PDA** to a terminal state. No further admin-gated instructions can succeed.

**Sample program**:
One of two reference implementations shipped with the library. `admin-authority-sample` shows the macro-driven path (`#[admin_authority]` plus a separate `admin_initialize`). `admin-authority-sample-manual` shows the manual-init path (`AdminConfig::initialize` called inside the consumer's own `initialize`). Both contain `update_value` gated by `#[require_admin]`, `admin_transfer`, and `admin_renounce`, with integration tests.
_Avoid_: demo, example (implies optional; the samples are hard RFP deliverables)

## Example dialogue

> **Dev:** "Can two different users each be admin of the same program?"
> **Domain expert:** "No. There's one admin per program. If you want separate admin domains, each consumer deploys their own ELF. They get their own Config PDA and their own admin."

> **Dev:** "What happens if the admin loses their key after renouncing?"
> **Domain expert:** "Nothing. Renounce is terminal. The Config PDA is frozen. Admin-gated instructions are permanently blocked. Design the program so renounce is only callable when that's the intended outcome."
