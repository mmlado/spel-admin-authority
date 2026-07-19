use admin_authority::require_admin;
use spel_framework::prelude::*;

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ProgramConfig {
    pub value: u64,
}

#[lez_program]
#[admin_authority]
mod admin_authority_sample {
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
            vec![admin_config, caller, config],
            vec![],
        ))
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

        let output = admin_authority_sample::update_value(admin_config, caller, config, 42)
            .expect("admin call must succeed");

        let post = &output.post_states[2];
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

        let err = admin_authority_sample::update_value(admin_config, caller, config, 42)
            .expect_err("signer is not the current admin");
        assert!(matches!(err, SpelError::Unauthorized { .. }));
    }

    #[test]
    fn update_value_rejects_uninitialized_config() {
        let caller = acct(1, true);
        let admin_config = acct(9, false);

        let config = acct(5, false);

        let err = admin_authority_sample::update_value(admin_config, caller, config, 42)
            .expect_err("admin authority not initialized");
        assert!(matches!(err, SpelError::Unauthorized { .. }));
    }

    #[test]
    fn update_value_rejects_after_renounce_() {
        let caller = acct(1, true);
        let mut admin_config = acct(9, false);
        AdminConfig::bootstrap(&mut admin_config, AdminCandidate::Signer, &caller).unwrap();
        AdminConfig::perform_renounce(&mut admin_config, &caller).unwrap();

        let config = acct(5, false);

        let err = admin_authority_sample::update_value(admin_config, caller, config, 42)
            .expect_err("admin authority renounced");
        assert!(matches!(err, SpelError::Unauthorized { .. }));
    }
}
