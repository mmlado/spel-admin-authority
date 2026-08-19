//! Single-admin authority primitive for LEZ programs. Tracks the current
//! admin in a Config PDA and provides transfer, renounce, and gate-check
//! operations consumed by the `#[require_admin]` and `#[admin_authority]`
//! macros.

#![warn(missing_docs)]

use spel_framework::prelude::*;

pub use admin_authority_macros::{admin_authority, admin_initialize, instruction, require_admin};

mod config;
mod errors;
pub use config::*;
pub use errors::*;

extern crate self as admin_authority;

/// Creates the admin Config PDA and installs the caller as admin
/// (self-election).
///
/// Must be called once per program deployment. Re-initialization is rejected
/// automatically by `#[account(init)]`. There is no candidate argument at
/// initialize: LEZ rejects a transaction whose account list contains the same
/// account id twice, so a caller could never pass itself again as evidence.
/// See ADR-0005. To hand the role to an external keyholder or PDA, call
/// `admin_transfer` after initializing.
///
/// The Config PDA does not exist until this instruction lands, so any caller
/// can win the race during the initialization window. LEZ deployment
/// transactions carry no instructions, so deploy and initialize cannot be
/// bundled today; send `admin_initialize` immediately after deployment.
#[instruction]
pub fn admin_initialize(
    #[account(init, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
) -> SpelResult {
    AdminConfig::bootstrap(&mut config, AdminCandidate::Signer, &caller)?;
    Ok(SpelOutput::execute(
        vec![
            (
                config.account,
                AutoClaim::Claimed(Claim::Pda(PdaSeed::new(seed_from_str("admin_config")))),
            ),
            (caller.account, AutoClaim::None),
        ],
        vec![],
    ))
}

/// Replaces the current admin with a new signer or PDA.
///
/// Only the current admin can call. The new admin is described by the
/// `AdminCandidate` and validated against `new_account`. After this
/// transaction lands, the previous admin can no longer call gated
/// instructions.
///
/// Transfer-to-self is impossible: `caller` and `new_account` would
/// share one account id, which LEZ rejects as a duplicate. That is
/// acceptable because such a transfer would be a no-op.
#[instruction]
pub fn admin_transfer(
    #[account(mut, pda = literal("admin_config"))] mut admin_config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
    new_account: AccountWithMetadata,
    candidate: ::admin_authority::AdminCandidate,
    offset: usize,
) -> SpelResult {
    AdminConfig::perform_transfer_at(&mut admin_config, offset, &caller, candidate, &new_account)?;
    Ok(SpelOutput::execute(
        vec![admin_config.account, caller.account, new_account.account],
        vec![],
    ))
}

/// Permanently zeros the admin in the Config PDA.
///
/// Only the current admin can call. Writes `AccountId::default()` to the
/// Config PDA, blocking all future admin-gated instructions. Terminal,
/// there is no recovery path by design.
#[instruction]
pub fn admin_renounce(
    #[account(mut, pda = literal("admin_config"))] mut admin_config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
    offset: usize,
) -> SpelResult {
    AdminConfig::perform_renounce_at(&mut admin_config, offset, &caller)?;
    Ok(SpelOutput::execute(
        vec![admin_config.account, caller.account],
        vec![],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ADR-0010 alignment self-test. The probe fn hands the wrapper every
    // role kwarg this test knows about: a name the macro rejects is a
    // compile error here, in our own CI, not in a consumer's build. The
    // runtime half asserts the manifest declares exactly the same set,
    // so adding an inject account without teaching the macro (or the
    // reverse) fails this test either way.
    #[test]
    fn wrapper_kwargs_match_declared_inject_accounts() {
        #[require_admin(admin_config = a, caller = b)]
        fn __probe(a: AccountWithMetadata, b: AccountWithMetadata) -> Result<(), SpelError> {
            let _ = (&a, &b);
            Ok(())
        }

        let specs = spel_framework_core::extension::read_inject_specs(std::path::Path::new(env!(
            "CARGO_MANIFEST_DIR"
        )))
        .expect("inject metadata must parse");
        let mut by_wrapper: Vec<(String, Vec<&str>)> = specs
            .iter()
            .map(|s| {
                let mut roles: Vec<&str> = s.accounts.iter().map(|a| a.role.as_str()).collect();
                roles.sort_unstable();
                (s.wrapper.clone(), roles)
            })
            .collect();
        by_wrapper.sort();
        assert_eq!(
            by_wrapper,
            vec![
                (
                    "admin_initialize".to_string(),
                    vec!["admin_config", "caller"]
                ),
                ("require_admin".to_string(), vec!["admin_config", "caller"]),
            ],
            "manifest inject accounts drifted from the wrapper's kwarg set"
        );
    }
}
