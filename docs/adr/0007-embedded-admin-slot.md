---
status: accepted
---

# Embedded admin slot: born initialized, marker as single source of truth

Embedded mode, `#[admin_authority(admin_config = prog_config, offset = 32)]`, stores the 32-byte admin slot inside an existing consumer account at a byte offset instead of the dedicated `admin_config` PDA. The kwargs follow the established role contract, key is the role name, value is the consumer's account name. The declaration is program-wide and rewrites the `admin_config` role's inject entry: the role injects the named consumer account, with the canonical constraint copied from the consumer's account-creating declaration minus `init` and `mut`, instead of the dedicated PDA param. Gated instructions that declare the embedding account use it via skip-if-declared, ones that do not get it injected PDA-verified. The rewrite is per-role, not per-spec: the remaining inject accounts (`caller`, and any other extension's roles) keep injecting as before. Bare `#[admin_authority]` keeps dedicated mode unchanged. Internally dedicated mode is the degenerate case `offset = 0`, one code path for both modes. Embedded mode removes one account from every gated transaction, which is its quantifiable benefit in the overhead notes.

## Born initialized, no `admin_initialize`

Embedded mode emits no `admin_initialize`. The consumer's own account-creating instruction, the one carrying `#[account(init)]` on the embedding account, writes the initial admin via bootstrap. This is what keeps terminal renounce sound: the slot is provably non-zero from the moment the account exists, so an all-zeros slot unambiguously means Renounced. Reinit rejection rides the consumer account's own `#[account(init)]`, and the admin-side front-running window does not exist in embedded mode. A consumer that creates the embedding account without bootstrapping the slot has shipped a permanently renounced program, and the error message says so.

Rejected: a discriminator byte in the slot (uninit, active, renounced). It solves the same ambiguity but diverges from dedicated mode's bare 32-byte layout and charges every consumer a layout change when switching modes. Rejected: forbidding renounce in embedded mode, which amputates the API instead of answering the question.

## Management instructions keep working

The discovered `admin_transfer` and `admin_renounce` must work in embedded mode, and freeze-authority's instructions get the same treatment, so the machinery is library-agnostic framework work, not admin-specific codegen.

- **Role substitution on discovered fns.** The marker kwarg makes the framework replace the discovered fn's `admin_config` param with the consumer's account, name and constraint. The constraint is copied from the consumer's canonical declaration, the account-creating instruction's `#[account(init, pda = ...)]`. Conflicting declarations across instructions are a compile error naming both sites. Substitution only touches fns of extensions that opt in via `bound_args`.
- **Marker-bound const args.** Extension metadata (`[[package.metadata.spel.bound_args]]` with `arg`, `from`, `default`) declares a trailing fn param bound to a module-marker kwarg. The framework appends the literal at every dispatch call site and excludes it from the IDL. The offset is never a caller-supplied instruction arg, that would be a caller-controlled write location.

Rejected: emitting no management instructions in embedded mode and having the consumer hand-write transfer and renounce. Smaller surface, but it forks the instruction set between modes and freeze would inherit the same fork for seven instructions.

## Windowed primitives live on `AuthoritySlot`

Reads slice `data[offset..offset+32]`, writes splice exactly those bytes and leave surrounding consumer data untouched, too-short data is a loud `SlotOutOfBounds` error and never NotInitialized. These primitives (`read_at`, `write_at`) live in spel-authority so admin and freeze share one implementation of the most dangerous code in the feature. The primitive stays slot-only, each config type owns adjacent bytes such as freeze's `is_frozen` bool. The consumer embeds the slot as a real `AuthoritySlot` field with only fixed-size fields before it, a dynamic prefix would make the offset undecidable.

Rejected: whole-data `write_to` against the embedding account, which would overwrite the consumer's neighboring fields.

## Marker is the only writer of location kwargs

In embedded mode a consumer-written `admin_config` or `offset` kwarg on `#[require_admin]` is a compile error, it could only contradict the program-wide declaration. The framework's own gate stamping is the sole source of those kwargs. The embedding account is declared under the marker's name in every instruction that declares it. The `caller` kwarg stays allowed, signer naming is orthogonal to slot location.

## Consequences

- The proof vehicle is `admin-authority-sample-embedded` at a non-zero offset, including a regression test that a neighboring field survives `admin_transfer` (the splice must not trample consumer data). Dedicated-mode samples' dry-run output must stay byte-identical, since dedicated mode became `offset = 0` internally.
- The fork's wrapper-kwarg contract (wrapper args are inject-account names) gains one non-role kwarg, `offset`, and needs a one-line amendment.
- Expansion ordering is load-bearing: the injection pass runs on the consumer-authored gate attr, then the framework stamps the location kwargs. Consumer-authored args disable injection per the existing rule, framework-stamped args do not. Stamping before injecting would make every embedded gate look manual and silently stop `caller` injection.
- Freeze-authority adopts both mechanisms for its own config and instructions. The authority-suite libraries move in lockstep, so no cross-version combination ships.
