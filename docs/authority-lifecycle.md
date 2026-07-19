# Authority Lifecycle

How `AdminConfig` moves between states over a program's lifetime, what each transition validates, and which guarantees the library provides.

> Status at M1: this document specifies the target behavior. The validation and transition logic it describes (`AdminCandidate::validate_with_account`, the method bodies behind each transition) lands in M2; at this milestone the instruction fns are stubs.

## State machine

```
        ┌──────────────────┐
        │  Uninitialized   │   Config PDA does not exist
        └────────┬─────────┘
                 │ admin_initialize
                 ▼
        ┌──────────────────┐
        │   Initialized    │   admin = <set>
        │  admin = AcctId  │◄────────┐
        └────┬─────────────┘         │
             │                       │ admin_transfer
             │ admin_transfer        │
             │                       │
             ▼                       │
        ┌──────────────────┐         │
        │   Initialized    │─────────┘
        │ admin = AcctId'  │
        └────┬─────────────┘
             │ admin_renounce
             ▼
        ┌──────────────────┐
        │    Renounced     │   admin = AccountId::default()
        └──────────────────┘   terminal; no further transitions
```

## States

**Uninitialized.** The Config PDA at `(program_id, "admin_config")` does not yet exist on-chain. Any caller can submit `admin_initialize`. This period is the **initialization window**.

**Initialized.** The Config PDA exists and `admin` holds the current admin's `AccountId`. Every `#[require_admin]`-gated instruction reads this value to authorize the caller.

**Renounced.** The Config PDA exists but `admin` is `AccountId::default()`. This is a terminal state. All gated instructions reject with `AdminError::Renounced`, and `admin_transfer` and `admin_renounce` both fail.

## Transitions

### `admin_initialize`

```
Uninitialized → Initialized
```

**Inputs:** `caller: AccountWithMetadata` (signer).

**Resolution:** self-election, always. The caller becomes admin (`admin = caller.account_id`). There is no candidate argument at initialize: the LEZ duplicate-account rule rejects any transaction listing the same account id twice, so a caller could never pass itself again as candidate evidence (see [ADR-0005](adr/0005-self-election-via-caller.md)). To hand the role to an external keyholder or PDA, initialize first and then call `admin_transfer`.

**Validations:** the Config PDA is enforced as freshly initialized by `#[account(init)]`, the caller's `is_authorized` flag must be true, and the caller's `account_id` must not be `AccountId::default()` (which would put the config into Renounced state at birth).

**Failure modes:** `AdminError::InvalidCandidate`.

### `admin_transfer`

```
Initialized → Initialized (new admin)
```

**Inputs:** `caller: AccountWithMetadata` (the current admin, signing), `new_admin_account: AccountWithMetadata`, `new_admin: AdminCandidate`.

**Validations:** the config must not be Renounced, `caller.account_id` must equal the stored `admin`, `caller.is_authorized` must be true, and `AdminCandidate::validate_with_account(new_admin_account)` must succeed (either a co-signed signer or a program-owned PDA whose address matches the derived one). A candidate resolving to the default `AccountId` is rejected as `InvalidCandidate`: that value is the renounced sentinel, and installing it would be a silent renounce.

**Failure modes:** `AdminError::NotAdmin`, `AdminError::Renounced`, `AdminError::InvalidCandidate`, `AdminError::UndeployedPda`, `AdminError::CandidateMismatch`.

### `admin_renounce`

```
Initialized → Renounced
```

**Inputs:** `caller: AccountWithMetadata` (the current admin, signing).

**Effect:** writes `AccountId::default()` into `admin`. This is terminal; no further transitions are possible.

**Validations:** the config must be Initialized, `caller.account_id` must equal the stored `admin`, and `caller.is_authorized` must be true.

**Failure modes:** `AdminError::NotAdmin`, `AdminError::Renounced`.

## State detection

Two states have empty-looking config slots in different ways. Uninitialized means the PDA contains no data; Renounced means it contains data but the admin field is zeroed.

| State | `account.data` | `admin` |
|---|---|---|
| Uninitialized | empty | n/a (decode fails) |
| Initialized | non-empty | non-default |
| Renounced | non-empty | `AccountId::default()` |

`AdminConfig::assert_admin` discriminates in order:

1. If `account.data` is empty, return `AdminError::NotInitialized`.
2. If decode succeeds and `admin == AccountId::default()`, return `AdminError::Renounced`.
3. Otherwise compare `signer.account_id` to `admin`.

The two error variants are deliberately separate because a consumer may want different UX for each. "You haven't called init yet" is a recoverable mistake; "Admin renounced" is permanent.

## Reinit rejection

The Config PDA is created with `#[account(init)]`. LEZ's `validate_execution` rule rejects any post-state where the pre-account was already non-default but the instruction declared `init`. So once `admin_initialize` succeeds, no second call can succeed, even after renounce. The PDA address is fixed at `(program_id, "admin_config")`, so there is no second address to initialize.

## Signer validation

The `is_authorized` flag on `AccountWithMetadata` is set by LEZ during transaction validation. It is true if and only if the transaction's `WitnessSet` contains a valid signature over the tx body by the AccountId's keypair. Library methods that take a signer check this flag instead of re-implementing signature verification. SPEL's `#[account(signer)]` constraint emits the same check automatically before the handler runs; the library checks again at its own boundary to enforce the invariant defensively.

For `AdminCandidate::Signer`, `new_admin_account.is_authorized` must also be true. Without this, an attacker could name an arbitrary AccountId as the new admin. Requiring a co-signature proves the new admin's keyholder consents to the transfer.

For `AdminCandidate::Pda`, signatures aren't applicable. PDAs cannot sign. The library proves the candidate by deriving the address from `(program_id, seed)`, checking it matches `new_admin_account.account_id`, and checking the PDA is program-owned (`program_owner` is not the default `ProgramId`; an untouched account is also rejected). Anyone can fund the derived address, but only the owning program's claim stamps `program_owner`, so a funded-but-unclaimed account is rejected as undeployed.

## Program-as-admin via CPI

A PDA admin invokes a gated instruction through its owning program. The owning program builds a chained_call to the gated instruction, includes the admin PDA in the call's account list, and declares `caller-pda-seeds = seed`. LEZ verifies that `AccountId::for_public_pda(caller_program_id, seed)` matches `PDA.account_id`. If it does, LEZ propagates `is_authorized = true` to the callee, and the gated instruction's `#[require_admin]` check passes just as it would for an EOA admin.

Only the owning program can produce a valid seed claim, because LEZ pins `caller_program_id` to the actual caller. The PDA must already be deployed when `admin_transfer` accepts it; otherwise the candidate is rejected.

## Initialization window risk

Between deployment and the first successful `admin_initialize`, anyone can submit `admin_initialize` and become admin. Deployers must send `admin_initialize` immediately after deployment. Bundling with the deployment is not possible today: a LEZ deployment transaction carries only bytecode, no instructions and no accounts.

The library provides no protection against front-running this window. By construction, the Config PDA does not yet exist, so there is no stored authority to check against, and LEZ records no deployer identity to gate on.

## Renounce is terminal

`admin_renounce` writes `AccountId::default()` and that ends it. There is no `admin_recover`, no `admin_reinit`. A recoverable renounce is not a renounce. Programs that need a pause or freeze semantic should implement that separately; admin authority is the wrong primitive for it.

If the admin loses their key before renouncing, that program's gated instructions become permanently uncallable. The end state is the same as renounce, reached accidentally. There is no recovery, by design, because any recovery path would also be an exploit path.
