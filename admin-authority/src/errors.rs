//! The library error type and its conversions from the shared
//! `authority` errors and into `SpelError` at the SPEL boundary.

#![warn(missing_docs)]

use authority::AuthorityError;
use spel_framework::prelude::*;

/// Errors returned by `admin-authority` library methods. Mapped to
/// `SpelError::Unauthorized` at the SPEL boundary so the lib stays
/// independent of the framework's error surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    /// Config PDA data is empty; `admin_initialize` has not been called.
    NotInitialized,
    /// Stored `slot` holder is `AccountId::default()`; admin is renounced.
    Renounced,
    /// Signer's `account_id` does not match the stored `slot` holder.
    NotAdmin,
    /// Signer is not authorized (no valid signature in the WitnessSet).
    MissingSignature,
    /// Candidate failed validation: `Signer` did not co-sign, or the
    /// resolved id is the default `AccountId` (installing it would be a
    /// silent renounce).
    InvalidCandidate,
    /// `AdminCandidate::Pda` references an account no program owns
    /// (unclaimed or merely funded).
    UndeployedPda,
    /// Candidate's derived address does not match `new_account.account_id`.
    CandidateMismatch,
    /// Borsh encoding of `AdminConfig` failed.
    EncodingFailed,
    /// Borsh decoding of `AdminConfig` failed.
    DecodingFailed,
    /// Error in writing data
    AccountDataTooLarge,
    /// An embedded-slot window `[offset..offset+32)` does not fit inside
    /// the account's data. Layout error: the declared offset and the
    /// account's actual size disagree.
    SlotOutOfBounds,
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
            AdminError::SlotOutOfBounds => write!(f, "embedded slot window out of bounds"),
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

impl From<AuthorityError> for AdminError {
    fn from(e: AuthorityError) -> Self {
        match e {
            AuthorityError::InvalidCandidate => AdminError::InvalidCandidate,
            AuthorityError::UndeployedPda => AdminError::UndeployedPda,
            AuthorityError::CandidateMismatch => AdminError::CandidateMismatch,
            AuthorityError::NotHolder => AdminError::NotAdmin,
            AuthorityError::Renounced => AdminError::Renounced,
            AuthorityError::MissingSignature => AdminError::MissingSignature,
            AuthorityError::SlotOutOfBounds => AdminError::SlotOutOfBounds,
        }
    }
}

#[cfg(test)]
mod test {
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
