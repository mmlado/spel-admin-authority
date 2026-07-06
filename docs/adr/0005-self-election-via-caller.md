# `admin_initialize` self-elects the caller; candidates are transfer-time only

LEZ rejects any transaction whose account list contains the same account id twice, before program code runs (`logos-execution-zone`, `lee/state_machine/src/validated_state_diff/mod.rs:57-61`, the same check guards `public_account_ids` at line 324). The old `admin_initialize(config, caller, new_admin_account, new_admin)` documented self-election as "pass your own account as both caller and evidence", which produces exactly that duplicate. The documented primary use case could never land. Found by cross-reading the mint-authority signing gist (0x-r4bbit, `ead1077de7b5ab6a5ed67b8b6a69f5bc`) against this library.

We drop the candidate pair from initialize entirely: `admin_initialize(config, caller)` installs the signing caller as Admin. An external keyholder or PDA becomes Admin through a subsequent `admin_transfer`, which keeps the claim-plus-evidence pair unchanged. `AdminConfig::bootstrap` also stays unchanged, so consumers doing single-transaction setup inside their own `initialize` handler can still install any admin by passing a distinct evidence account (the `admin-authority-sample-manual` pattern).

This matches the token program's direction (gist Option D: the authority is always its own distinct account and the client signs exactly that account) and our own glossary, which had always defined `AdminCandidate` as a transfer-time argument.

## Considered Options

**1. Self-election only (chosen).**
Smallest privileged API. Self-election is the common case and the initialization race makes electing anyone but yourself in a single unbundled transaction pointless anyway. External admin costs one extra transaction.

**2. Keep the candidate form, add `admin_initialize_self`.**
Preserves single-transaction PDA-admin init through the injected instruction. Rejected: doubles the initialize surface (the gist's Option C tradeoff) to serve a case already covered by `bootstrap` in a consumer handler or by init-then-transfer.

**3. Bare `AccountId` argument instead of the evidence account.**
Would dodge the duplicate rule without dropping the candidate. Rejected: cannot validate key liveness or PDA deployment, so a typo bricks the program. Contradicts the RFP reliability requirement and the existing glossary rule.

**4. Framework-level optional accounts or client-side dedupe.**
Rejected as wrong altitude: a framework and node change to serve one instruction's ergonomics.

## Consequences

- Transfer-to-self is impossible under the same rule (caller and evidence would share one id). Acceptable: it would be a no-op.
- The injected `admin_initialize` shrinks from four accounts to two. IDL fixtures and clients regenerate.
- The old doc-comment advice "bundle this call with deployment" was never implementable: LEZ deployment is a dedicated transaction type carrying no instructions and no accounts. The initialization window (front-running race) is structural today, so guidance stays "send `admin_initialize` immediately after deployment" and the race is accepted on testnet. Should LEZ ever bundle an initialization transaction with deployment, self-election fits it naturally: the deployer signs the bundled init and becomes Admin. A genesis-admin mitigation (deployer key baked into the binary as a const, checked at init) was considered and skipped.
- Regression coverage is end-to-end only: a self-election transaction against a local node, red on the old shape, green on the new. No LEZ crates enter this repo as dependencies. The node-side rule is evidenced by the source reference above.
- freeze-authority has a sharper version of the same problem: admin-as-freeze-authority is unreachable through any instruction sequence, with no two-step escape. Decision deferred, recorded in spel-freeze-authority `CONTEXT.md` under flagged ambiguities.
