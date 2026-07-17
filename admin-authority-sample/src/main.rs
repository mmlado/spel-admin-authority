use admin_authority::require_admin;
use spel_framework::prelude::*;

#[lez_program]
#[admin_authority]
mod admin_authority_sample {
    use super::*;

    #[instruction]
    #[require_admin]
    pub fn update_value(
        #[account(pda = literal("admin_config"))] admin_config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut _config: AccountWithMetadata,
        _new_value: u64,
    ) -> SpelResult {
        todo!()
    }
}
