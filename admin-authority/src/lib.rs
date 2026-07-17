//! Single-admin authority primitive for LEZ programs. Tracks the current
//! admin in a Config PDA and provides transfer, renounce, and gate-check
//! operations consumed by the `#[require_admin]` and `#[admin_authority]`
//! macros.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use spel_framework::prelude::*;

pub use admin_authority_macros::{admin_authority, instruction, require_admin};

extern crate self as admin_authority;

/// Transfer-time argument describing the intended new admin.
///
/// Paired with `new_admin_account: AccountWithMetadata` at every transfer.
/// `AdminCandidate` is the claim, `AccountWithMetadata` is the chain-state
/// evidence. One without the other provides no security guarantee.
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum AdminCandidate {
    /// New admin is a keyholder. Validated by checking
    /// `new_admin_account.is_authorized == true` (co-signed the tx).
    Signer,
    /// New admin is a program-owned PDA. Validated by deriving the address
    /// from `(program_id, seed)`, matching it against `new_admin_account`,
    /// and confirming the PDA is initialized.
    Pda {
        program_id: ProgramId,
        seed: [u8; 32],
    },
}

/// On-chain admin authority state for a single program.
///
/// Stored in the program's Config PDA at `(program_id, "admin_config")`.
/// Created once via `admin_initialize`; cannot be reinitialized.
/// `admin == AccountId::default()` indicates the renounced state.
#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdminConfig {
    /// Current admin's `AccountId`. `AccountId::default()` means renounced.
    pub admin: AccountId,
}

/// Errors returned by `admin-authority` library methods. Mapped to
/// `SpelError::Unauthorized` at the SPEL boundary so the lib stays
/// independent of the framework's error surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    /// Config PDA data is empty; `admin_initialize` has not been called.
    NotInitialized,
    /// Stored `admin` is `AccountId::default()`; admin is renounced.
    Renounced,
    /// Signer's `account_id` does not match the stored `admin`.
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
        SpelError::Unauthorized {
            message: e.to_string(),
        }
    }
}

/// Creates the admin Config PDA and installs the caller as the first admin
/// (self-election).
///
/// There is no candidate argument: LEZ rejects a transaction whose account
/// list contains the same account id twice, so a caller could never pass
/// itself again as candidate evidence. The signing caller becomes admin;
/// to hand the role to an external keyholder or PDA, call `admin_transfer`
/// after initializing.
///
/// Must be called once per program deployment. Re-initialization is rejected
/// automatically by `#[account(init)]`. The Config PDA does not exist until
/// this instruction lands, so any caller can win the race during the
/// initialization window. Send it immediately after deployment; bundling is
/// not possible (a LEZ deployment transaction carries no instructions).
#[instruction]
pub fn admin_initialize(
    #[account(init, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
) -> SpelResult {
    todo!()
}

/// Replaces the current admin with a new signer or PDA.
///
/// Only the current admin can call. The new admin is described by the
/// `AdminCandidate` and validated against `new_admin_account`. After this
/// transaction lands, the previous admin can no longer call gated
/// instructions.
#[instruction]
pub fn admin_transfer(
    #[account(mut, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
    _new_admin_account: AccountWithMetadata,
    _new_admin: ::admin_authority::AdminCandidate,
) -> SpelResult {
    todo!()
}

/// Permanently zeros the admin in the Config PDA.
///
/// Only the current admin can call. Writes `AccountId::default()` to the
/// Config PDA, blocking all future admin-gated instructions. Terminal,
/// there is no recovery path by design.
#[instruction]
pub fn admin_renounce(
    #[account(mut, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
) -> SpelResult {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_error_display_strings() {
        assert_eq!(
            AdminError::NotInitialized.to_string(),
            "admin authority not initialized"
        );
        assert_eq!(
            AdminError::Renounced.to_string(),
            "admin authority renounced"
        );
        assert_eq!(
            AdminError::NotAdmin.to_string(),
            "signer is not the current admin"
        );
        assert_eq!(
            AdminError::MissingSignature.to_string(),
            "admin signature missing"
        );
        assert_eq!(
            AdminError::InvalidCandidate.to_string(),
            "invalid admin candidate"
        );
        assert_eq!(
            AdminError::UndeployedPda.to_string(),
            "candidate PDA is not deployed"
        );
        assert_eq!(
            AdminError::CandidateMismatch.to_string(),
            "candidate address mismatch"
        );
        assert_eq!(
            AdminError::EncodingFailed.to_string(),
            "AdminConfig encoding failed"
        );
        assert_eq!(
            AdminError::DecodingFailed.to_string(),
            "AdminConfig decoding failed"
        );
        assert_eq!(
            AdminError::AccountDataTooLarge.to_string(),
            "AdminConfig too large for account data"
        );
    }

    #[test]
    fn admin_error_maps_to_unauthorized() {
        let spel: SpelError = AdminError::NotAdmin.into();
        match spel {
            SpelError::Unauthorized { message } => {
                assert_eq!(message, "signer is not the current admin");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn admin_error_renounced_maps_to_unauthorized_with_message() {
        let spel: SpelError = AdminError::Renounced.into();
        match spel {
            SpelError::Unauthorized { message } => {
                assert_eq!(message, "admin authority renounced");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }
}
