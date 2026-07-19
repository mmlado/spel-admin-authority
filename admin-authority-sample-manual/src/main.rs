use admin_authority::require_admin;
use spel_framework::prelude::*;

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ProgramConfig {
    pub value: u64,
}

#[lez_program]
mod admin_authority_sample_manual {
    use admin_authority::AdminConfig;

    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("admin_config"))] mut admin_config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        #[account(init, pda = literal("program_config"))] prog_config: AccountWithMetadata,
    ) -> SpelResult {
        AdminConfig::bootstrap(
            &mut admin_config,
            admin_authority::AdminCandidate::Signer,
            &caller,
        )?;
        Ok(SpelOutput::execute(
            vec![admin_config, prog_config, caller],
            vec![],
        ))
    }

    #[instruction]
    #[require_admin]
    pub fn update_value(
        #[account(pda = literal("admin_config"))] admin_config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut config: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        let state = ProgramConfig { value: new_value };
        config.account.data = borsh::to_vec(&state)
            .map_err(|_| SpelError::SerializationError {
                message: "encoding failed".into(),
            })?
            .try_into()
            .map_err(|_| SpelError::SerializationError {
                message: "data too large".into(),
            })?;
        Ok(SpelOutput::execute(
            vec![admin_config, config, caller],
            vec![],
        ))
    }

    #[instruction]
    #[require_admin(config = config)]
    pub fn admin_transfer(
        #[account(mut, pda = literal("admin_config"))] mut config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        new_admin_account: AccountWithMetadata,
        new_admin: ::admin_authority::AdminCandidate,
    ) -> SpelResult {
        AdminConfig::perform_transfer(&mut config, &caller, new_admin, &new_admin_account)?;
        Ok(SpelOutput::execute(
            vec![config, caller, new_admin_account],
            vec![],
        ))
    }

    #[instruction]
    #[require_admin(config = config)]
    pub fn admin_renounce(
        #[account(mut, pda = literal("admin_config"))] mut config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
    ) -> SpelResult {
        AdminConfig::perform_renounce(&mut config, &caller)?;
        Ok(SpelOutput::execute(vec![config, caller], vec![]))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use admin_authority::{AdminCandidate, AdminConfig};

    fn acct(id_byte: u8, signed: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account::default(),
            is_authorized: signed,
            account_id: AccountId::new([id_byte; 32]),
        }
    }

    #[test]
    fn update_value_succeeds_for_admin() {
        let caller = acct(1, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();
        let config = acct(5, false);

        let output = admin_authority_sample_manual::update_value(admin_config, caller, config, 42)
            .expect("admin call must succeed");

        let post = &output.post_states[1];
        let state = ProgramConfig::try_from_slice(post.account().data.as_ref()).unwrap();
        assert_eq!(state.value, 42);
    }

    #[test]
    fn update_value_rejects_non_admin() {
        let admin = acct(1, true);
        let caller = acct(2, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &admin).unwrap();
        let config = acct(5, false);

        let err = admin_authority_sample_manual::update_value(admin_config, caller, config, 42)
            .expect_err("signer is not the current admin");
        match err {
            SpelError::Unauthorized { message } => {
                assert_eq!(message, "signer is not the current admin")
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn update_value_rejects_uninitialized_config() {
        let caller = acct(1, true);
        let admin_config = acct(9, false);

        let config = acct(5, false);

        let err = admin_authority_sample_manual::update_value(admin_config, caller, config, 42)
            .expect_err("admin authority not initialized");
        match err {
            SpelError::Unauthorized { message } => {
                assert_eq!(message, "admin authority not initialized")
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn update_value_rejects_after_renounce_() {
        let caller = acct(1, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();
        AdminConfig::perform_renounce(&mut admin_config, &caller).unwrap();

        let config = acct(5, false);

        let err = admin_authority_sample_manual::update_value(admin_config, caller, config, 42)
            .expect_err("admin authority renounced");
        match err {
            SpelError::Unauthorized { message } => {
                assert_eq!(message, "admin authority renounced")
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn admin_transfer_updates_admin() {
        let caller = acct(1, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();

        let new_admin = acct(2, true);

        let _ = admin_authority_sample_manual::admin_transfer(
            admin_config,
            caller,
            new_admin,
            AdminCandidate::Signer,
        )
        .expect("transfer call must succeed");
    }

    #[test]
    fn admin_transfer_rejects_non_admin() {
        let caller = acct(1, true);
        let non_admin = acct(4, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();

        let new_admin = acct(2, true);

        let err = admin_authority_sample_manual::admin_transfer(
            admin_config,
            non_admin,
            new_admin,
            AdminCandidate::Signer,
        )
        .expect_err("signer is not the current admin");
        assert!(matches!(err, SpelError::Unauthorized { .. }));
    }

    #[test]
    fn admin_renounce_then_transfer_fails() {
        let caller = acct(1, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();

        let new_admin = acct(2, true);
        let output =
            admin_authority_sample_manual::admin_renounce(admin_config.clone(), caller.clone())
                .expect("renounce must succeed");

        let mut renounced = admin_config;
        renounced.account = output.post_states[0].account().clone();

        let err = admin_authority_sample_manual::admin_transfer(
            renounced,
            caller,
            new_admin,
            AdminCandidate::Signer,
        )
        .expect_err("admin authority renounced");
        assert!(matches!(err, SpelError::Unauthorized { .. }));
    }
}
