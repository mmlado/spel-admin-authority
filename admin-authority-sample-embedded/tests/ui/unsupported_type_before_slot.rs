// A dynamically sized field before the slot makes the offset
// undecidable. The derived const must refuse this build.
use admin_authority::AdminConfig;
use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default)]
pub struct BadConfig {
    pub name: String,
    #[admin_slot]
    pub admin: AdminConfig,
}

fn main() {}
