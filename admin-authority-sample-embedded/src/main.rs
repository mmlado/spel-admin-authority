use admin_authority::{AdminCandidate, AdminConfig};
use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

/// The admin slot lives inside this account at byte offset 32:
/// value 0..8, padding 8..32, admin 32..64. The non-zero offset is
/// deliberate, it exercises the splice.
#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct ProgConfig {
    pub value: u64,
    pub padding: [u8; 24],
    pub admin: AdminConfig,
}

impl ProgConfig {
    fn from_account(account: &AccountWithMetadata) -> Result<Self, SpelError> {
        Self::try_from_slice(&account.account.data).map_err(|_| SpelError::SerializationError {
            message: "decoding failed".into(),
        })
    }

    fn write_to(&self, account: &mut AccountWithMetadata) -> Result<(), SpelError> {
        account.account.data = borsh::to_vec(self)
            .map_err(|_| SpelError::SerializationError {
                message: "encoding failed".into(),
            })?
            .try_into()
            .map_err(|_| SpelError::SerializationError {
                message: "data too large".into(),
            })?;
        Ok(())
    }
}

#[lez_program]
#[admin_authority(admin_config = config, offset = 32)]
mod admin_authority_sample_embedded {
    use admin_authority::require_admin;

    /// Creates the embedding account and bootstraps the admin slot in
    /// the same instruction: the slot is born initialized, there is no
    /// admin_initialize in embedded mode.
    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("prog_config"))] mut config: AccountWithMetadata,
        #[account(signer)] signer: AccountWithMetadata,
    ) -> SpelResult {
        ProgConfig {
            value: 0,
            padding: [0; 24],
            admin: AdminConfig::default(),
        }
        .write_to(&mut config)?;
        AdminConfig::bootstrap_at(&mut config, 32, AdminCandidate::Signer, &signer)?;
        Ok(SpelOutput::execute(
            vec![config.account, signer.account],
            vec![],
        ))
    }

    /// Gated. The embedding account is declared, so injection skips it
    /// and the gate reads the slot at offset 32 from this very param.
    #[require_admin]
    #[instruction]
    pub fn update_value(
        #[account(mut, pda = literal("prog_config"))] mut config: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        let mut state = ProgConfig::from_account(&config)?;
        state.value = new_value;
        state.write_to(&mut config)?;
        Ok(SpelOutput::execute(vec![config.account], vec![]))
    }

    /// Gated but does not declare the embedding account: injection must
    /// synthesize prog_config, PDA-verified with the canonical constraint.
    #[require_admin]
    #[instruction]
    pub fn poke() -> SpelResult {
        Ok(SpelOutput::execute(vec![], vec![]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(id_byte: u8, signed: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account::default(),
            is_authorized: signed,
            account_id: AccountId::new([id_byte; 32]),
        }
    }

    // The sample-level splice regression: consumer state written next to
    // the slot survives a transfer through the real ProgConfig layout.
    #[test]
    fn value_survives_admin_transfer() {
        let mut config_account = acct(9, false);
        let old_admin = acct(1, true);
        let new_admin = acct(2, true);

        ProgConfig {
            value: 7,
            padding: [0xAB; 24],
            admin: AdminConfig::default(),
        }
        .write_to(&mut config_account)
        .expect("initial write");
        AdminConfig::bootstrap_at(&mut config_account, 32, AdminCandidate::Signer, &old_admin)
            .expect("bootstrap");

        AdminConfig::perform_transfer_at(
            &mut config_account,
            32,
            &old_admin,
            AdminCandidate::Signer,
            &new_admin,
        )
        .expect("transfer");

        let state = ProgConfig::from_account(&config_account).expect("decode after transfer");
        assert_eq!(state.value, 7, "consumer value trampled by the transfer");
        assert_eq!(
            state.padding, [0xAB; 24],
            "padding trampled by the transfer"
        );
        assert!(state.admin.assert_admin(&new_admin).is_ok());
        assert!(state.admin.assert_admin(&old_admin).is_err());
    }

    // Born renounced: creating the account without bootstrapping leaves a
    // slot that rejects everyone, permanently.
    #[test]
    fn skipped_bootstrap_is_born_renounced() {
        let mut config_account = acct(9, false);
        let anyone = acct(3, true);
        ProgConfig {
            value: 0,
            padding: [0; 24],
            admin: AdminConfig::default(),
        }
        .write_to(&mut config_account)
        .expect("initial write");

        let cfg = AdminConfig::from_account_at(&config_account, 32).expect("decode");
        assert!(
            cfg.assert_admin(&anyone).is_err(),
            "born renounced slot must reject"
        );
    }
}
