use spel_framework::prelude::*;
use admin_authority::{admin_authority, require_admin};

#[lez_program]
#[admin_authority]
mod admin_authority_sample {
    use super::*;

    #[instruction]
    #[require_admin]
    pub fn update_value(
        #[account(pda = literal("admin_config"))] admin_config: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        todo!()
    }
}
