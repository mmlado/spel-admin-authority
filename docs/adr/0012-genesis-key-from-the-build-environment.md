# A genesis key from the build environment gates `admin_initialize`

**Status:** accepted.

`admin_initialize` installs whoever calls it first, and nothing can be bundled with a LEZ v0.2.4 deployment, so the first caller after a deploy wins. The program id is the image id, so every build that changes an input starts a fresh race.

A consumer can now compile a genesis key into the build: `SPEL_ADMIN_GENESIS`, 64 hexadecimal characters, read with `option_env!` while the consumer's guest builds. With a key, only that account may run `admin_initialize` (`AdminError::NotGenesisAdmin`). Without one, the first caller still becomes admin. A malformed or all-zero key fails the build, and a build script declares the variable so cargo rebuilds the crate when it changes.

The key applies to the dedicated `admin_initialize` only. In embedded mode the caller of the consumer's account-creating instruction becomes admin, and gating that instruction is the consumer's job. `require_genesis(&caller)` gives it the same check against the same variable.

## Considered Options

**1. Env var, opt-in (chosen).** No framework change, and builds without a key behave as before.

**2. Marker kwarg, `#[admin_authority(genesis = "...")]`.** Keeps the key in source. Deferred: SPEL v0.7.0 markers accept only `offset = <integer>` and `<role> = <account>`, so this needs an upstream change first.

**3. A required key.** Unset would mean nobody can initialize. Rejected for now: every build, samples and tests included, would need a key.

**4. Gate on the deployer.** Not possible on LEZ v0.2.4: a deployment transaction carries no signer, so the program cannot know who deployed it.

## Consequences

- The key is part of the image, so builds with different keys have different program ids and different PDAs.
- Cargo's `[env]` table in `.cargo/config.toml` sets the same variable from a repository.
