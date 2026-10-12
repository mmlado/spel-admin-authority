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

The two-step handover moves the admin without leaving Initialized:

```
   Initialized (admin = A)
        │ admin_nominate(N), signed by A
        ▼
   Initialized (admin = A, nominee = N)      admin_transfer, admin_renounce and
        │ admin_accept, signed by N          admin_nominate(all-zero id) clear N
        ▼
   Initialized (admin = N)
```

## States

**Uninitialized.** The Config PDA at `(program_id, "admin_config")` does not yet exist on-chain. Any caller can submit `admin_initialize`, or only the genesis key when the build carries one. This period is the **initialization window**.

**Initialized.** The Config PDA exists and `admin` holds the current admin's `AccountId`. Every `#[require_admin]`-gated instruction reads this value to authorize the caller.

**Renounced.** The Config PDA exists but `admin` is `AccountId::default()`. This is a terminal state. All gated instructions reject with `AdminError::Renounced`, and `admin_transfer` and `admin_renounce` both fail.

## Transitions

### `admin_initialize`

```
Uninitialized → Initialized
```

**Inputs:** `caller: AccountWithMetadata` (signer).

**Resolution:** self-election, always. The caller becomes admin (`admin = caller.account_id`). There is no candidate argument at initialize: the LEZ duplicate-account rule rejects any transaction listing the same account id twice, so a caller could never pass itself again as candidate evidence (see [ADR-0005](adr/0005-self-election-via-caller.md)). To hand the role to an external keyholder or PDA, initialize first and then call `admin_transfer`.

**Validations:** with a genesis key in the build, the caller must be that key. The Config PDA is enforced as freshly initialized by `#[account(init)]`, the caller's `is_authorized` flag must be true, some program must own the caller's account, and the caller's `account_id` must not be `AccountId::default()` (which would put the config into Renounced state at birth).

**Failure modes:** `AdminError::NotGenesisAdmin`, `AdminError::InvalidCandidate`, `AdminError::UnownedSigner`.

### `admin_transfer`

```
Initialized → Initialized (new admin)
```

**Inputs:** `caller: AccountWithMetadata` (the current admin, signing), `new_account: AccountWithMetadata`, `candidate: AdminCandidate`.

**Validations:** the config must not be Renounced, `caller.account_id` must equal the stored `admin`, `caller.is_authorized` must be true, and `AdminCandidate::validate_with_account(new_account)` must succeed (either a co-signed signer or a program-owned PDA whose address matches the derived one). A candidate resolving to the default `AccountId` is rejected as `InvalidCandidate`: that value is the renounced sentinel, and installing it would be a silent renounce.

**Effect:** installs the new admin and clears any pending nominee.

**Failure modes:** `AdminError::NotAdmin`, `AdminError::Renounced`, `AdminError::InvalidCandidate`, `AdminError::UnownedSigner`, `AdminError::UndeployedPda`, `AdminError::CandidateMismatch`.

### `admin_nominate`

```
Initialized → Initialized (nominee set)
```

**Inputs:** `caller: AccountWithMetadata` (the current admin, signing), `new_admin: [u8; 32]`.

**Effect:** stores the nominee next to the admin, replacing any earlier one. The all-zero id cancels. The admin does not change.

**Validations:** same as the caller half of `admin_transfer`. Nothing is checked about the nominee: it is a bare id, and a mistyped one can never accept.

**Failure modes:** `AdminError::NotAdmin`, `AdminError::Renounced`, `AdminError::MissingSignature`.

### `admin_accept`

```
Initialized (nominee set) → Initialized (new admin)
```

**Inputs:** `caller: AccountWithMetadata` (the nominee, signing for itself, or a PDA nominee signed through its owning program's chained call).

**Effect:** the nominee becomes admin, and the nominee field is cleared.

**Validations:** the config must not be Renounced, a nominee must be stored, `caller.is_authorized` must be true, `caller.account_id` must equal the nominee, and some program must own the caller's account.

**Failure modes:** `AdminError::Renounced`, `AdminError::NoNomination`, `AdminError::MissingSignature`, `AdminError::NotNominee`, `AdminError::UnownedSigner`.

### `admin_renounce`

```
Initialized → Renounced
```

**Inputs:** `caller: AccountWithMetadata` (the current admin, signing).

**Effect:** writes `AccountId::default()` into `admin` and clears any pending nominee. This is terminal; no further transitions are possible.

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

For `AdminCandidate::Signer`, `new_account.is_authorized` must also be true. Without this, an attacker could name an arbitrary AccountId as the new admin. Requiring a co-signature proves the new admin's keyholder consents to the transfer. The same applies to `admin_accept`, where the nominee's own signature is the consent.

A `Signer` that becomes admin, at `admin_initialize`, as a transfer target or at `admin_accept`, must also be owned by some program. On LEZ v0.2.1 to v0.2.4 an account no program owns can sign once and is then rejected in every later transaction, so the library refuses it with `AdminError::UnownedSigner`. `wallet auth-transfer init` gives a key that owner.

For `AdminCandidate::Pda`, signatures aren't applicable. PDAs cannot sign. The library proves the candidate by deriving the address from `(program_id, seed)`, checking it matches `new_account.account_id`, and checking the PDA is program-owned (`program_owner` is not the default `ProgramId`; an untouched account is also rejected). Anyone can fund the derived address, but only the owning program's claim stamps `program_owner`, so a funded-but-unclaimed account is rejected as undeployed.

## Program-as-admin via CPI

A PDA admin invokes a gated instruction through its owning program. The owning program builds a chained_call to the gated instruction, includes the admin PDA in the call's account list, and declares `caller-pda-seeds = seed`. LEZ verifies that `AccountId::for_public_pda(caller_program_id, seed)` matches `PDA.account_id`. If it does, LEZ propagates `is_authorized = true` to the callee, and the gated instruction's `#[require_admin]` check passes just as it would for an EOA admin.

Only the owning program can produce a valid seed claim, because LEZ pins `caller_program_id` to the actual caller. The PDA must already be deployed when `admin_transfer` accepts it; otherwise the candidate is rejected. A PDA nominee takes the role the same way: its owning program chained-calls `admin_accept` with the PDA's seeds.

## Initialization window risk

Between deployment and the first successful `admin_initialize`, anyone can submit `admin_initialize` and become admin, unless the build carries a genesis key (`SPEL_ADMIN_GENESIS`, ADR-0012). Then only that key can, and anyone else gets `AdminError::NotGenesisAdmin`. Bundling with the deployment is not possible today: a LEZ deployment transaction carries only bytecode, no instructions and no accounts, and so no deployer identity to gate on.

In embedded mode the same window sits on the consumer's account-creating instruction. `require_genesis(&caller)` applies the genesis key there.

## Renounce is terminal

`admin_renounce` writes `AccountId::default()` and that ends it. There is no `admin_recover`, no `admin_reinit`. A recoverable renounce is not a renounce. Programs that need a pause or freeze semantic should implement that separately; admin authority is the wrong primitive for it.

If the admin loses their key before renouncing, that program's gated instructions become permanently uncallable. The end state is the same as renounce, reached accidentally. There is no recovery, by design, because any recovery path would also be an exploit path.
