use spel_framework::prelude::*;

#[lez_program]
#[admin_authority]
mod admin_authority_sample {
    use super::*;

    #[instruction]
    pub fn update_value(
        #[account(pda = literal("admin_config"))] admin_config: AccountWithMetadata,
        #[account(mut, pda = literal("program_config"))] mut config: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        new_value: u64,
    ) -> SpelResult {
        todo!()
    }
}
