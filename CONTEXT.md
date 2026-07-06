# Admin Authority

A SPEL library that brings the Solidity Ownable pattern to LEZ programs. One admin per program deployment, and admin-gated instructions reject all other callers.

## Language

**Program**:
A stateless ELF binary deployed to LEZ. Identified by its image_id (hash of the binary). Each unique binary has a unique program_id and an isolated PDA namespace.
_Avoid_: contract, smart contract

**Admin**:
The single `AccountId` stored in the Config PDA that is authorized to call admin-gated instructions. Set at `admin_initialize` by Self-election: the caller becomes the Admin. An external keyholder or PDA becomes Admin via a subsequent `admin_transfer`. Transferable or permanently renounced.
_Avoid_: owner (Solidity term), authority (ambiguous with LEZ's `is_authorized`)

**Self-election**:
The only `admin_initialize` path: the signing caller is installed as Admin, and no candidate argument exists at initialize. Forced by the Duplicate-account rule, since a caller listing itself again as candidate evidence would duplicate its account id.
_Avoid_: describing initialize as taking an `AdminCandidate` (that is transfer-time only)

**Duplicate-account rule**:
LEZ invariant: a transaction whose account list contains the same account id twice is rejected before execution (`ValidatedStateDiff::from_public_transaction`). Shapes every API that pairs a caller account with a candidate account, because the two can never be the same account in one transaction.
_Avoid_: designing instructions that expect one account to appear in two parameter slots

**Config PDA**:
The on-chain account that stores the `AdminConfig` state. Derived from `(program_id, "admin_config")`. Created once via `admin_initialize`, and cannot be reinitialized.
_Avoid_: admin account, state account, "config" (too generic, since consumers may already claim that seed for unrelated program state)

**Admin-gated instruction**:
A SPEL instruction annotated with `#[require_admin]`. The check (decode Config PDA + `assert_admin`) is injected into the top of the handler body by **re-expanding** the `#[require_admin]` proc-macro on the handler the framework emits, so a non-Admin caller is rejected before the handler's own logic runs. Gate an instruction when its body does not already enforce admin itself. Management instructions built on `perform_transfer` / `perform_renounce` carry the check inside, so gating them is redundant (a second decode of the same PDA). The manual sample gates them anyway to demonstrate the `config = ...` name override; that is a documentation choice, not a requirement. See [ADR-0004](docs/adr/0004-require-admin-injection-contract.md).
_Avoid_: protected instruction, restricted instruction; "the validator checks admin" (the generic `#[account]` validator checks account _shape_, not Admin _identity_ — that is the injected prologue's job)

**Wrapper (injection contract)**:
A per-instruction gate proc-macro (`#[require_admin]`, freeze-authority's `#[require_not_frozen]`) whose **check** is injected by re-expansion. The framework leaves the Wrapper attr on the emitted handler instead of stripping it, so it re-expands and prepends its prologue. A Wrapper resolves its target parameter names from its own attribute arguments with conventional defaults, `#[require_admin(config = admin_config, signer = caller)]`. A consumer with differently-named params passes the args explicitly. A Wrapper never reads or strips `#[account]`. That attribute belongs to the framework, which reads it for the validator and IDL and strips all of it once. Multiple Wrappers on one instruction each inject a prologue block and never conflict. The gate **accounts** are separate from the check: the consumer declares them, or the framework injects the missing ones from metadata at parse time (see Param injection and [ADR-0006](docs/adr/0006-param-injection-and-relaxed-mode.md)). See [ADR-0004](docs/adr/0004-require-admin-injection-contract.md) for the check.
_Avoid_: having a Wrapper scrape `#[account]` attrs to find its idents (couples it to the framework's private attribute and breaks once the framework strips them); conflating the check injection (the Wrapper's job, by re-expansion) with the account injection (the framework's job, from metadata)

**Renounce**:
Permanent, irreversible removal of admin authority. Zeros `admin` to `AccountId::default()` in the Config PDA. There is no recovery path and no reinit possible (the PDA still exists, and `#[account(init)]` rejects reuse).
_Avoid_: revoke (ambiguous; could imply reversibility), burn

**AdminError**:
Custom error enum in the `admin-authority` library crate. Library methods return `AdminError`, and instruction handlers map it to `SpelError::Unauthorized` at the SPEL boundary (method bodies are stubs at M1, the returns land in M2). This keeps the library independent of SPEL's error types.
_Avoid_: returning `SpelError` directly from library methods

**AdminCandidate**:
Transfer-time argument describing the intended new admin. `Signer` carries no data; validation checks `new_admin_account.is_authorized` (co-signed the tx). `Pda { program_id, seed }` is validated by deriving the address via `AccountId::for_public_pda` and confirming the PDA is initialized. Distinct from `AdminConfig.admin_authority`, which stores only the resolved `AccountId`. Always paired with a `new_admin_account: AccountWithMetadata` parameter; `AdminCandidate` is the claim, `AccountWithMetadata` is the chain-state evidence. One without the other provides no security guarantee. Consequence of the Duplicate-account rule: transfer-to-self is impossible (caller and evidence would share one id), which is acceptable because it would be a no-op.
_Avoid_: using a bare `AccountId` arg for transfer (cannot validate key ownership or PDA existence)

**Transfer history**:
Not recorded on-chain in this release. The current admin is always readable from the Config PDA; historical transfers require an off-chain indexer. Future improvement: once `lez-events` (LP-0012) lands in an official LEZ release, the library will emit typed events (`AdminInitialized`, `AdminTransferred`, `AdminRenounced`) from its methods. No extra accounts, and queryable via `getTransactionReceipt`.

**Initialization window**:
The period between program deployment and the first call to `admin_initialize`. During this window the Config PDA does not exist and any caller can front-run and become admin. Deployers must call `admin_initialize` immediately after deployment. Bundling with the deployment is not possible today (a LEZ deployment transaction carries no instructions), so the race is structural and accepted on testnet.
_Avoid_: "setup phase" (too vague)

**Declared gate params**:
The explicit style: the consumer writes the `admin_config` and `caller` params on a gated instruction, and the gate check re-expands to reference them. A fully declared handler expands byte-identically to an injected one. The manual sample ships this style, while the macro sample ships the injected style, so the pair documents both. See [ADR-0006](docs/adr/0006-param-injection-and-relaxed-mode.md).
_Avoid_: "strict mode" as a build mode (superseded; declaration is a style choice, not a mode)

**Attribute-order convention (`#[require_admin]` + `#[instruction]`)**:
No longer required. It mattered when `#[require_admin]` scraped `#[account]` params for shape validation, so it had to run before the `#[instruction]` shim stripped them. Since `#[require_admin]` now reads `config`/`signer` from attribute args and references only the param idents (which the shim leaves intact), the order of `#[require_admin]` and `#[instruction]` no longer changes the result.
_Avoid_: reintroducing an ordering rule for a macro that no longer reads `#[account]`

**Param injection**:
The framework synthesizes a gate's missing account params at parse time, driven by the extension's `[[package.metadata.spel.inject]]` metadata. Always active, skip-if-declared: a declared param is never touched, and an injected param is exactly what the declaration would have been, so the two styles produce the same program. Runs in every IDL producer so the IDLs never diverge. Whether release builds should require explicit declaration instead is an open question for the framework maintainers. See [ADR-0006](docs/adr/0006-param-injection-and-relaxed-mode.md).
_Avoid_: "relaxed mode" (superseded framing); the old `SPEL_ADMIN_AUTHORITY_RELAXED` env var

**Injected instructions**:
The three instructions added to a consumer's module by `#[admin_authority]`: `admin_initialize`, `admin_transfer`, `admin_renounce`. They appear in the IDL and are callable via SPEL CLI. Source lives as real `#[instruction] fn` definitions in `admin-authority/src/lib.rs`; the framework discovers them via a path-dep scan triggered by the `#[admin_authority]` marker and emits cross-crate dispatch calls (`::admin_authority::admin_initialize(...)`) into the consumer's binary. They never exist as copy-pasted source in the consumer module.
_Avoid_: "generated functions" (implies they exist as source in the consumer); "synthesized templates" (was true pre-pivot, no longer accurate); "admin_init" or "init_admin" (wrong prefix/suffix convention)

**Extension attr (framework discovery mechanism)**:
The trigger by which the SPEL framework discovers a third-party library's instructions. Each extension library declares its marker attribute name in `[package.metadata.spel.extension_attr]` in its `Cargo.toml`. When a consumer's `#[lez_program]` module carries that attribute, the framework scans the library's `src/lib.rs` for `#[instruction]`-annotated fns and includes them in the consumer's dispatcher + IDL. `admin-authority` declares `extension_attr = "admin_authority"`, so `#[admin_authority]` on a consumer mod triggers the scan. Framework is library-agnostic; the same mechanism powers any future extension (e.g. `freeze-authority`).
_Avoid_: "plugin", "hook" (imply runtime registration; this is compile-time discovery)

**Caller vs Submitter**:
Two roles the word "caller" tends to conflate. The Caller is the `#[account(signer)]` param inside an instruction, the account whose `is_authorized` the admin check asserts. For `admin_transfer` the Caller is always the current Admin. The Submitter is whoever posts the signed transaction to the sequencer, and can be anyone holding the fully signed blob, including the new admin or a third party. Validation reads the witness set, never the transport.
_Avoid_: "caller" for the party submitting the transaction (that is the Submitter)

**Witness exchange**:
The off-chain flow that produces a multi-signature transaction, needed because a `Signer`-candidate transfer requires both the current Admin and the new admin to sign one message. The Caller builds the message once (fetching one nonce per signer, since the protocol pairs `message.nonces[i]` with `witness[i]` positionally), signs it, and exports a partial-transaction blob. The co-signer's CLI decodes the blob against the IDL, shows what is being signed, appends its witness, and either returns the blob or submits it. The blob expires as soon as any included signer's nonce changes on chain.
_Avoid_: dual-build (both machines constructing the message independently; nonce drift breaks the signatures); blind signing (co-signer must see the decoded instruction before signing)

**admin-authority-macros (sub-crate)**:
Proc-macro crate that ships alongside the `admin-authority` library. Provides `#[admin_authority]` (marker, pass-through), `#[require_admin]` (reads `config`/`signer` attribute args and prepends the runtime check by re-expansion), and an internal `#[instruction]` shim that strips `#[account(...)]` helper attrs so the library's own source compiles in isolation. Required because attribute macros must live in a `proc-macro = true` crate, separate from the runtime library.
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
