use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use spel_framework::prelude::*;

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum AdminCandidate {
    Signer,
    Pda { program_id: ProgramId, seed: [u8; 32] },
}