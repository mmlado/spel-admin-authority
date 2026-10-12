//! The library error type and its conversions from the shared
//! `authority` errors and into `SpelError` at the SPEL boundary.

#![warn(missing_docs)]

use authority::AuthorityError;
use spel_framework::prelude::*;

/// Errors returned by `admin-authority` library methods. Each maps to
/// `SpelError::Custom` with a stable code from [`AdminError::code`], which
/// LEZ reports as program error `6000 + code`.
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
    /// The admin key, or a `Signer` transfer target, is owned by no
    /// program. LEZ would strand it after one transaction. Run
    /// `wallet auth-transfer init` on the key first.
    UnownedSigner,
    /// `admin_accept` was called with no nominee in the config.
    NoNomination,
    /// The signer of `admin_accept` is not the nominee.
    NotNominee,
    /// The build carries a genesis key (`SPEL_ADMIN_GENESIS`) and the
    /// caller of `admin_initialize` is not it.
    NotGenesisAdmin,
    /// The config account is not owned by the program reading it.
    ConfigNotOwned,
}

impl AdminError {
    /// The stable code of this error, in the 1xx block.
    pub const fn code(&self) -> u32 {
        match self {
            AdminError::NotInitialized => 101,
            AdminError::Renounced => 102,
            AdminError::NotAdmin => 103,
            AdminError::MissingSignature => 104,
            AdminError::InvalidCandidate => 105,
            AdminError::UndeployedPda => 106,
            AdminError::CandidateMismatch => 107,
            AdminError::EncodingFailed => 108,
            AdminError::DecodingFailed => 109,
            AdminError::AccountDataTooLarge => 110,
            AdminError::SlotOutOfBounds => 111,
            AdminError::UnownedSigner => 112,
            AdminError::NoNomination => 113,
            AdminError::NotNominee => 114,
            AdminError::NotGenesisAdmin => 115,
            AdminError::ConfigNotOwned => 116,
        }
    }
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
            AdminError::UnownedSigner => write!(f, "admin key is owned by no program"),
            AdminError::NoNomination => write!(f, "no admin nominee to accept"),
            AdminError::NotNominee => write!(f, "signer is not the admin nominee"),
            AdminError::NotGenesisAdmin => write!(f, "signer is not this build's genesis admin"),
            AdminError::ConfigNotOwned => write!(f, "admin config is not owned by this program"),
        }
    }
}

impl From<AdminError> for SpelError {
    fn from(e: AdminError) -> Self {
        SpelError::custom(e.code(), e.to_string())
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
            AuthorityError::UnownedSigner => AdminError::UnownedSigner,
            AuthorityError::NoNomination => AdminError::NoNomination,
            AuthorityError::NotNominee => AdminError::NotNominee,
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

    // Codes are API: clients branch on them, so a renumbering must fail here.
    #[test]
    fn codes_are_stable() {
        use AdminError::*;
        let pinned = [
            (NotInitialized, 101),
            (Renounced, 102),
            (NotAdmin, 103),
            (MissingSignature, 104),
            (InvalidCandidate, 105),
            (UndeployedPda, 106),
            (CandidateMismatch, 107),
            (EncodingFailed, 108),
            (DecodingFailed, 109),
            (AccountDataTooLarge, 110),
            (SlotOutOfBounds, 111),
            (UnownedSigner, 112),
            (NoNomination, 113),
            (NotNominee, 114),
            (NotGenesisAdmin, 115),
            (ConfigNotOwned, 116),
        ];
        for (err, code) in pinned {
            assert_eq!(err.code(), code, "{err:?}");
        }
    }

    #[test]
    fn spel_error_carries_the_code_and_the_message() {
        let spel: SpelError = AdminError::NotAdmin.into();
        assert_eq!(spel.error_code(), 6103);
        match spel {
            SpelError::Custom { code, message } => {
                assert_eq!(code, 103);
                assert_eq!(message, "signer is not the current admin");
            }
            other => panic!("expected Custom, got {other:?}"),
        }
    }
}
