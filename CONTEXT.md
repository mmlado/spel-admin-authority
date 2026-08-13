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
The on-chain account that stores the `AdminConfig` state in dedicated mode. Derived from `(program_id, "admin_config")`. Created once via `admin_initialize`, and cannot be reinitialized.
_Avoid_: admin account, state account, "config" (too generic, since consumers may already claim that seed for unrelated program state)

**Dedicated mode / Embedded mode**:
Where the admin slot lives. The marker is bare `#[admin_authority]` in both modes; the mode is inferred (ADR-0013). Dedicated mode is the default: admin slot in the Config PDA. Embedded mode stores the 32-byte admin slot inside an existing consumer account, and three declarations select it, each on the thing it describes: `#[admin_slot]` on the consumer struct's field (the offset derives from it, ADR-0011), `#[admin_initialize]` on the account-creating instruction (its `#[account(init)]` param is the embedding account; with several, the anchor's `admin_config = <param>` kwarg names the one), and nothing on the marker. The slot marker and the anchor must agree: a marked field with no anchored fn refuses to compile (the slot would ship born renounced), an anchored fn with no marked field refuses too (the derivation has no carrier). An embedded declaration rewrites the `admin_config` role's inject entry. Instead of injecting the dedicated PDA param, the role injects the named consumer account with the canonical constraint copied from the consumer's account-creating declaration, minus `init` and minus `mut` since the gate only reads. A gated instruction that declares the embedding account uses it via skip-if-declared, one that does not (a gated instruction need not touch program state) gets it injected PDA-verified like any other role. The rewrite is per-role, not per-spec: every other inject account (`caller` now, freeze's `freeze_account` in its M3) keeps injecting as before. The old marker role kwarg is retired: writing `#[admin_authority(admin_config = ...)]` is a hard compile error naming the fix.

Embedded mode emits no `admin_initialize`: the slot is born initialized. The consumer's own account-creating instruction (the one carrying `#[account(init)]` on the embedding account) writes the initial admin via `AdminConfig::bootstrap`. Consequences: an all-zeros embedded slot unambiguously means Renounced (never "not yet initialized"), terminal renounce holds through the existing transfer/renounce guards, reinit rejection rides the consumer account's own `#[account(init)]`, and the admin-side front-running window does not exist in embedded mode. A consumer that creates the embedding account without bootstrapping the slot has shipped a permanently renounced program, that is their bug and the error message must say so.
The slot is read and written as a 32-byte window: reads slice `data[offset..offset+32]`, writes splice exactly those bytes and leave surrounding consumer data untouched. State discrimination stays three-way and keeps today's first check: empty data means the embedding account does not exist yet, that is NotInitialized. Non-empty data too short for the window is a loud layout error (the declared offset and the actual layout disagree). Zeros in the window mean Renounced, including born renounced, the consumer that created the embedding account without bootstrapping the slot. The consumer embeds the extension's config type as a real field in their own account type (`admin: AdminConfig` here, `freeze: FreezeConfig` for freeze-authority) and declares that field's borsh position as `offset`; the `authority` crate stays invisible to consumers. Every field before it must be fixed-size (a `Vec` or `String` before the slot makes the offset dynamic, which is forbidden). Internally dedicated mode is the degenerate case `offset = 0` over the Config PDA, one code path for both modes.

Management instructions keep working in embedded mode through two framework mechanisms, both library-agnostic so freeze-authority inherits them. **Role substitution on discovered fns**: the marker kwarg `admin_config = prog_config` makes the framework replace the discovered fn's `admin_config` param with the consumer's `prog_config`, name and constraint, where the constraint is copied from the consumer's own declaration (the account-creating instruction's `#[account(init, pda = ...)]` is the canonical one; conflicting declarations across instructions are a compile error naming both sites; none is a compile error). **Marker-bound const args**: extension metadata (`[[package.metadata.spel.bound_args]]` with `arg`, `from`, `default`) declares a trailing fn param bound to a module-marker kwarg; the framework appends the literal at every dispatch call site and excludes it from the IDL. The offset is never a caller-supplied instruction arg, that would be a caller-controlled write location. The windowed primitives live on `authority::AuthoritySlot` (`read_at` / `write_at` plus a `SlotOutOfBounds` error): one implementation of the slice and splice, shared by every library that embeds a slot. The slot primitive stays 32-byte and slot-only, each config type owns whatever extra bytes sit adjacent (freeze's `is_frozen` bool is freeze's business). The authority-suite milestones move in lockstep, admin and freeze adopt the mechanism in their own M3s.

In embedded mode the marker is the only writer of location kwargs: a consumer-written `admin_config` or `offset` kwarg on `#[require_admin]` is a compile error (it could only contradict the program-wide declaration), and the embedding account is declared under the marker's name in every instruction that declares it, no per-instruction renaming. The `caller` kwarg stays allowed, signer naming is orthogonal to slot location.
_Avoid_: `place` (say offset), per-instruction embedding declarations (the location is a program-wide property), emitting `admin_initialize` in embedded mode, whole-data `write_to` against an embedding account (splice only)

**Admin-gated instruction**:
A SPEL instruction annotated with `#[require_admin]`. The check (decode Config PDA + `assert_admin`) is injected into the top of the handler body by **re-expanding** the `#[require_admin]` proc-macro on the handler the framework emits, so a non-Admin caller is rejected before the handler's own logic runs. Gate an instruction when its body does not already enforce admin itself. Management instructions built on `perform_transfer` / `perform_renounce` carry the check inside, so gating them is redundant (a second decode of the same PDA). The manual sample gates them anyway to demonstrate the `admin_config = ...` name override; that is a documentation choice, not a requirement. See [ADR-0004](docs/adr/0004-require-admin-injection-contract.md).
_Avoid_: protected instruction, restricted instruction; "the validator checks admin" (the generic `#[account]` validator checks account _shape_, not Admin _identity_ — that is the injected prologue's job)

**Wrapper (injection contract)**:
A per-instruction gate proc-macro (`#[require_admin]`, freeze-authority's `#[require_not_frozen]`) whose **check** is injected by re-expansion. The framework leaves the Wrapper attr on the emitted handler instead of stripping it, so it re-expands and prepends its prologue. A Wrapper resolves its target parameter names from its own attribute arguments with conventional defaults; the kwarg keys are the inject-spec account names, `#[require_admin(admin_config = my_cfg, caller = owner)]`, plus the framework-stamped `offset` in embedded mode. A consumer with differently-named params passes the args explicitly. A Wrapper never reads or strips `#[account]`. That attribute belongs to the framework, which reads it for the validator and IDL and strips all of it once. Multiple Wrappers on one instruction each inject a prologue block and never conflict. The gate **accounts** are separate from the check: the consumer declares them, or the framework injects the missing ones from metadata at parse time (see Param injection and [ADR-0006](docs/adr/0006-param-injection-and-relaxed-mode.md)). See [ADR-0004](docs/adr/0004-require-admin-injection-contract.md) for the check.
_Avoid_: having a Wrapper scrape `#[account]` attrs to find its idents (couples it to the framework's private attribute and breaks once the framework strips them); conflating the check injection (the Wrapper's job, by re-expansion) with the account injection (the framework's job, from metadata)

**Renounce**:
Permanent, irreversible removal of admin authority. Zeros the admin slot to `AccountId::default()`. There is no recovery path and no reinit possible. In dedicated mode the Config PDA still exists and `#[account(init)]` rejects reuse. In embedded mode the slot is born initialized with the embedding account, so zeros are unambiguously this terminal state.
_Avoid_: revoke (ambiguous; could imply reversibility), burn

**AdminError**:
Custom error enum in the `admin-authority` library crate. Library methods return `AdminError`, and instruction handlers map it to `SpelError::Unauthorized` at the SPEL boundary (method bodies are stubs at M1, the returns land in M2). This keeps the library independent of SPEL's error types.
_Avoid_: returning `SpelError` directly from library methods

**AdminCandidate**:
`pub type AdminCandidate = authority::AuthorityCandidate` — a type alias onto the shared `authority` crate's primitive (extracted so admin-authority and freeze-authority share one implementation instead of duplicating it). Transfer-time argument describing the intended new admin. `Signer` carries no data; validation checks `new_account.is_authorized` (co-signed the tx). `Pda { program_id, seed }` is validated by deriving the address via `AccountId::for_public_pda` and confirming a program owns the account (funding alone does not count). A candidate resolving to the default `AccountId` is rejected, that value is the renounced sentinel. Distinct from `AdminConfig`'s stored holder, which is only the resolved `AccountId`. Always paired with a `new_account: AccountWithMetadata` parameter; `AdminCandidate` is the claim, `AccountWithMetadata` is the chain-state evidence. One without the other provides no security guarantee. Consequence of the Duplicate-account rule: transfer-to-self is impossible (caller and evidence would share one id), which is acceptable because it would be a no-op.
_See_: [authority CONTEXT](https://github.com/mmlado/spel-authority/blob/main/CONTEXT.md)
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

**Account layout**:
The Borsh-encoded shape of the bytes a program stores in one on-chain account, declared by putting `#[account_type]` on a struct or enum. Reaches the IDL's `accounts` section, the catalogue `spel inspect` decodes against and `decode_account_data_try_all` guesses from. `AdminConfig` is one in dedicated mode, where it is the whole content of the Config PDA. In embedded mode it is not: it occupies a window inside the consumer's own account, so the consumer's struct is the account layout and `AdminConfig` is a field of it.
_Avoid_: account type (ambiguous with the `AccountWithMetadata` parameter type), account struct

**Owned source**:
The consumer's own crate plus the local path dependencies it links. Code the program's author wrote or deliberately pulled in, so an `#[account_type]` there is taken at face value and becomes an account layout. Covers the `*_core` pattern, a workspace keeping its layouts in one crate and a thin program binary in another.
_Avoid_: local crate (a path dep may sit outside the workspace), first-party

**Activated extension**:
An extension library the consumer turned on by putting its marker attribute on the `#[lez_program]` module. Its `#[instruction]` functions are merged into the consumer's dispatcher, so they ship in the consumer's binary and run as part of the program. Its account layouts are therefore the program's own by fact rather than by courtesy: in dedicated mode `admin_transfer` and `admin_renounce` create and write the Config PDA holding `AdminConfig`, whether or not the consumer's own code ever names that type. In embedded mode the initializer is skipped and the slot is written into the consumer's account, so `AdminConfig` stops being an account layout and becomes a field of the consumer's.
_Avoid_: dependency (most dependencies are not extensions), discovered extension (discovery is the mechanism, activation is the consumer's choice), "the consumer trusts it" (the connection is that its code runs, not that it is trusted)

**Referenced type**:
A type described in the IDL because something already there has a field of that type, rather than because it carries an annotation. Lands in the IDL's `types` section. `AuthoritySlot` is one: `AdminConfig.slot` names it, so it is described even though the `authority` crate is never scanned for annotations. `AdminCandidate` is one by a second route, a type alias whose target is resolved and emitted under the alias name.
_Avoid_: helper type (says nothing about why it is there), transitive type

**Unowned crate**:
Any crate that is neither an owned source nor an activated extension, which is most of a program's dependency graph. Its `#[account_type]` annotations are inert: it cannot put an account layout into a program's IDL. Its types are still described when something the program uses references them.
_Avoid_: third-party (an activated extension is third-party too), external

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
