// #[admin_initialize] on an instruction that does not create the
// embedding account: the bootstrap must only ever run on a freshly
// created account, so this must refuse.
use admin_authority::{AdminCandidate, AdminConfig};
use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default)]
pub struct ProgConfig {
    pub value: u64,
    pub padding: [u8; 24],
    #[admin_slot]
    pub admin: AdminConfig,
}

#[lez_program]
#[admin_authority]
mod fixture {
    use admin_authority::{admin_initialize, require_admin};

    #[instruction]
    pub fn initialize(
        #[account(init, pda = literal("prog_config"))] mut config: AccountWithMetadata,
        #[account(signer)] signer: AccountWithMetadata,
    ) -> SpelResult {
        AdminConfig::bootstrap_at(&mut config, 32, AdminCandidate::Signer, &signer)?;
        Ok(SpelOutput::execute(
            vec![config.account, signer.account],
            vec![],
        ))
    }

    #[admin_initialize]
    #[instruction]
    pub fn poke() -> SpelResult {
        Ok(SpelOutput::execute(vec![], vec![]))
    }
}
