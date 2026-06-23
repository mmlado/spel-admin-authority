//! Single-admin authority primitive for LEZ programs. Tracks the current
//! admin in a Config PDA and provides transfer, renounce, and gate-check
//! operations consumed by the `#[require_admin]` and `#[admin_authority]`
//! macros.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use spel_framework::prelude::*;

/// Transfer-time argument describing the intended new admin.
///
/// Paired with `new_admin_account: AccountWithMetadata` at every transfer.
/// `AdminCandidate` is the claim; `AccountWithMetadata` is the chain-state
/// evidence. One without the other provides no security guarantee.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum AdminCandidate {
    /// New admin is a keyholder. Validated by checking
    /// `new_admin_account.is_authorized == true` (co-signed the tx).
    Signer,
    /// New admin is a program-owned PDA. Validated by deriving the address
    /// from `(program_id, seed)`, matching it against `new_admin_account`,
    /// and confirming the PDA is initialized.
    Pda { program_id: ProgramId, seed: [u8; 32] },
}

/// On-chain admin authority state for a single program.
///
/// Stored in the program's Config PDA at `(program_id, "admin_config")`.
/// Created once via `admin_initialize`; cannot be reinitialized.
/// `admin_authority == AccountId::default()` indicates the renounced state.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct AdminConfig {
    /// Current admin's `AccountId`. `AccountId::default()` means renounced.
    pub admin_authority: AccountId,
}

/// Errors returned by `admin-authority` library methods.
///
/// Library methods return `AdminError`. Instruction handlers map these to
/// `SpelError::Unauthorized` at the SPEL boundary so the library stays
/// independent of SPEL's error surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    /// Config PDA data is empty; `admin_initialize` has not been called.
    NotInitialized,
    /// Stored `admin_authority` is `AccountId::default()`; admin is renounced.
    Renounced,
    /// Signer's `account_id` does not match the stored `admin_authority`.
    NotAdmin,
    /// Signer is not authorized (no valid signature in the WitnessSet).
    MissingSignature,
    /// `AdminCandidate::Signer` paired with a default `AccountId`.
    InvalidCandidate,
    /// `AdminCandidate::Pda` references an undeployed PDA.
    UndeployedPda,
    /// Candidate's derived address does not match `new_admin_account.account_id`.
    CandidateMismatch,
    /// Borsh encoding of `AdminConfig` failed.
    EncodingFailed,
    /// Borsh decoding of `AdminConfig` failed.
    DecodingFailed,
    /// Error in writing data
    AccountDataTooLarge,
}

impl core::fmt::Display for AdminError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AdminError::NotInitialized => write!(f, "admin authority not initialized"),
            AdminError::Renounced => write!(f, "admin authority renounced"),
            AdminError::NotAdmin => write!(f, "signer is not the current admin"),
            AdminError::MissingSignature => write!(f, "admin signature missing"),
            AdminError::InvalidCandidate => write!(f, "invalid admin candidate"),
            AdminError::UndeployedPda => write!(f, "candidate PDA is not deployed"),
            AdminError::CandidateMismatch => write!(f, "candidate address mismatch"),
            AdminError::EncodingFailed => write!(f, "AdminConfig encoding failed"),
            AdminError::DecodingFailed => write!(f, "AdminConfig decoding failed"),
            AdminError::AccountDataTooLarge => write!(f, "AdminConfig too large for account data"),
        }
    }
}

impl From<AdminError> for SpelError {
    fn from(e: AdminError) -> Self {
        SpelError::Unauthorized { message: e.to_string() }
    }
}

impl AdminConfig {
    /// Constructs an initialized `AdminConfig`.
    ///
    /// Rejects `AccountId::default()` as the admin (reserved for renounced state).
    pub fn initialize(admin: AccountId) -> Result<Self, AdminError> {
        todo!()
    }

    /// Transfers admin authority to a new account.
    ///
    /// Validates `current` is the stored admin, then validates `candidate`
    /// against `new_account` and overwrites `admin_authority`.
    pub fn transfer(
        &mut self,
        current: &AccountWithMetadata,
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        todo!()
    }

    /// Permanently renounces admin authority.
    ///
    /// Zeros `admin_authority` to `AccountId::default()`. Terminal.
    pub fn renounce(&mut self, current: &AccountWithMetadata) -> Result<(), AdminError> {
        todo!()
    }

    /// Asserts the supplied signer is the current admin.
    ///
    /// Called by the `#[require_admin]` validator prologue. Checks: not
    /// renounced, `signer.account_id == admin_authority`,
    /// `signer.is_authorized == true`.
    pub fn assert_admin(&self, signer: &AccountWithMetadata) -> Result<(), AdminError> {
        todo!()
    }

    /// Borsh-encodes the state for storage in the Config PDA.
    pub fn encode(&self) -> Result<Vec<u8>, AdminError> {
        todo!()
    }

    /// Borsh-decodes the state from a byte slice.
    pub fn decode(data: &[u8]) -> Result<Self, AdminError> {
        todo!()
    }

    /// Decodes the state from an `AccountWithMetadata`.
    ///
    /// Returns `AdminError::NotInitialized` if the account's data is empty.
    pub fn from_account(account: &AccountWithMetadata) -> Result<Self, AdminError> {
        todo!()
    }

    pub fn write_to(&self, account: &mut AccountWithMetadata) -> Result<(), AdminError> {
        let bytes = self.encode()?;
        account.account.data = bytes.try_into().map_err(|_| AdminError::AccountDataTooLarge)?;
        Ok(())
    }
}

impl AdminCandidate {
    /// Validates the candidate against the supplied account.
    ///
    /// Returns the resolved `AccountId` to store as `admin_authority`.
    pub fn validate_with_account(
        &self,
        account: &AccountWithMetadata,
    ) -> Result<AccountId, AdminError> {
        todo!()
    }
}
