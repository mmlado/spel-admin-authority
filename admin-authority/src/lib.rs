//! Single-admin authority primitive for LEZ programs. Tracks the current
//! admin in a Config PDA and provides transfer, renounce, and gate-check
//! operations consumed by the `#[require_admin]` and `#[admin_authority]`
//! macros.

#![warn(missing_docs)]

use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

use authority::{AuthoritySlot, AuthorityCandidate, AuthorityError};

pub use admin_authority_macros::{admin_authority, instruction, require_admin};

extern crate self as admin_authority;

/// Transfer-time claim describing the intended new admin. Alias of the
/// shared [`AuthorityCandidate`]: `Signer` proves key control by
/// co-signature, `Pda { program_id, seed }` by address derivation plus a
/// deployment check. Always paired with a `new_admin_account` param that
/// carries the chain-state evidence.
pub type AdminCandidate = AuthorityCandidate;

/// On-chain admin authority state for a single program.
///
/// Stored in the program's Config PDA at `(program_id, "admin_config")`.
/// Created once via `admin_initialize`; cannot be reinitialized. Wraps the
/// shared [`AuthoritySlot`], whose holder is the current admin and whose
/// `AccountId::default()` sentinel marks the renounced state. The borsh
/// layout is a single 32-byte `AccountId`, unchanged by the extraction.
#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdminConfig {
    slot: AuthoritySlot,
}

impl AdminConfig {
    /// Constructs an initialised AdminConfig.
    ///
    /// Rejects `AccountId::default()` as the admin since that value is the
    /// reserved sentinel for the renounced state.
    pub fn initialize(admin: AccountId) -> Result<Self, AdminError> {
        Ok(Self { slot: AuthoritySlot::initialize(admin)? })
    }

    /// Asserts that the supplied signer is the current admin.
    ///
    /// Returns:
    /// - `AdminError::Renounced` if the config has no admin (terminal state).
    /// - `AdminError::MissingSignature` if the signer account did not sign the tx.
    /// - `AdminError::NotAdmin` if the signer signed but is not the stored admin.
    pub fn assert_admin(&self, signer: &AccountWithMetadata) -> Result<(), AdminError> {
        self.slot.assert(signer).map_err(Into::into)
    }

    /// Borsh-serialises the config for storage in the PDA's account data.
    ///
    /// Returns `AdminError::EncodingFailed` if serialisation fails.
    pub fn encode(&self) -> Result<Vec<u8>, AdminError> {
        borsh::to_vec(self).map_err(|_| AdminError::EncodingFailed)
    }

    /// Borsh-deserialises config from raw account data.
    ///
    /// Distinguishes "never initialised" from "corrupt":
    /// - `AdminError::NotInitialized` if `data` is empty (PDA exists but
    ///   `admin_initialize` has not run).
    /// - `AdminError::DecodingFailed` if `data` is non-empty but malformed.
    pub fn decode(data: &[u8]) -> Result<Self, AdminError> {
        if data.is_empty() {
            return Err(AdminError::NotInitialized);
        }
        Self::try_from_slice(data).map_err(|_| AdminError::DecodingFailed)
    }

    /// Loads config from an account's data field. Convenience wrapper over
    /// [`AdminConfig::decode`].
    pub fn from_account(account: &AccountWithMetadata) -> Result<Self, AdminError> {
        Self::decode(&account.account.data)
    }

    /// Replaces the current admin after authorising the caller and validating
    /// the incoming admin.
    pub fn transfer(
        &mut self,
        current: &AccountWithMetadata,
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        self.slot.assert(current)?;
        let next = candidate.validate(new_account)?;
        self.slot.transfer_to(next)?;
        Ok(())
    }

    /// Permanently zeros the admin to `AccountId::default()`, the renounced
    /// sentinel. Terminal: once renounced, `assert_admin` always fails with
    /// `AdminError::Renounced`, so this is idempotent and irreversible.
    ///
    /// Only the current admin may call (`assert_admin` runs first).
    pub fn renounce(&mut self, current: &AccountWithMetadata) -> Result<(), AdminError> {
        self.slot.assert(current)?;
        self.slot.renounce();
        Ok(())
    }

    /// Serialises and writes this config into an account's data field.
    ///
    /// Returns `AdminError:AccountDataTooLarge` if the encoded bytes exceed
    /// the account's max length.
    pub fn write_to(&self, account: &mut AccountWithMetadata) -> Result<(), AdminError> {
        let bytes = self.encode()?;
        account.account.data = bytes
            .try_into()
            .map_err(|_| AdminError::AccountDataTooLarge)?;
        Ok(())
    }

    /// Validates a candidate, builds a fresh config, and writes it to the PDA.
    ///
    /// Used by `admin_initialize` and by consumers doing single-tx deploy +
    /// admin setup inside their own `initialize` handler.
    pub fn bootstrap(
        config_account: &mut AccountWithMetadata,
        new_admin: AdminCandidate,
        new_admin_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let resolved = new_admin.validate(new_admin_account)?;
        let state = Self::initialize(resolved)?;
        state.write_to(config_account)
    }

    /// Loads config from account, transfers admin, writes back.
    pub fn perform_transfer(
        config_account: &mut AccountWithMetadata,
        current: &AccountWithMetadata,
        candidate: AdminCandidate,
        new_admin_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let mut state = Self::from_account(config_account)?;
        state.transfer(current, candidate, new_admin_account)?;
        state.write_to(config_account)
    }

    /// Loads config from account, renounce admin, writes back.
    pub fn perform_renounce(
        config_account: &mut AccountWithMetadata,
        current: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let mut state = Self::from_account(config_account)?;
        state.renounce(current)?;
        state.write_to(config_account)
    }
}

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
            AdminError::NotInitialized      => write!(f, "admin authority not initialized"),
            AdminError::Renounced           => write!(f, "admin authority renounced"),
            AdminError::NotAdmin            => write!(f, "signer is not the current admin"),
            AdminError::MissingSignature    => write!(f, "admin signature missing"),
            AdminError::InvalidCandidate    => write!(f, "invalid admin candidate"),
            AdminError::UndeployedPda       => write!(f, "candidate PDA is not deployed"),
            AdminError::CandidateMismatch   => write!(f, "candidate address mismatch"),
            AdminError::EncodingFailed      => write!(f, "AdminConfig encoding failed"),
            AdminError::DecodingFailed      => write!(f, "AdminConfig decoding failed"),
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

impl From<AuthorityError> for AdminError {
    fn from(e: AuthorityError) -> Self {
        match e {
            AuthorityError::InvalidCandidate    => AdminError::InvalidCandidate,
            AuthorityError::UndeployedPda       => AdminError::UndeployedPda,
            AuthorityError::CandidateMismatch   => AdminError::CandidateMismatch,
            AuthorityError::NotHolder           => AdminError::NotAdmin,
            AuthorityError::Renounced           => AdminError::Renounced,
            AuthorityError::MissingSignature    => AdminError::MissingSignature,
        }
    }
}

/// Creates the admin Config PDA and installs the caller as admin
/// (self-election).
///
/// Must be called once per program deployment. Re-initialization is rejected
/// automatically by `#[account(init)]`. There is no candidate argument at
/// initialize: LEZ rejects a transaction whose account list contains the same
/// account id twice, so a caller could never pass itself again as evidence.
/// See ADR-0005. To hand the role to an external keyholder or PDA, call
/// `admin_transfer` after initializing.
///
/// The Config PDA does not exist until this instruction lands, so any caller
/// can win the race during the initialization window. LEZ deployment
/// transactions carry no instructions, so deploy and initialize cannot be
/// bundled today; send `admin_initialize` immediately after deployment.
#[instruction]
pub fn admin_initialize(
    #[account(init, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
) -> SpelResult {
    AdminConfig::bootstrap(&mut config, AdminCandidate::Signer, &caller)?;
    Ok(SpelOutput::execute(
        vec![
            (
                config.account,
                AutoClaim::Claimed(Claim::Pda(PdaSeed::new(seed_from_str("admin_config")))),
            ),
            (caller.account, AutoClaim::None),
        ],
        vec![],
    ))
}

/// Replaces the current admin with a new signer or PDA.
///
/// Only the current admin can call. The new admin is described by the
/// `AdminCandidate` and validated against `new_admin_account`. After this
/// transaction lands, the previous admin can no longer call gated
/// instructions.
///
/// Transfer-to-self is impossible: `caller` and `new_admin_account` would
/// share one account id, which LEZ rejects as a duplicate. That is
/// acceptable because such a transfer would be a no-op.
#[instruction]
pub fn admin_transfer(
    #[account(mut, pda = literal("admin_config"))] mut config: AccountWithMetadata,
    #[account(signer)] caller: AccountWithMetadata,
    new_admin_account: AccountWithMetadata,
    new_admin: ::admin_authority::AdminCandidate,
) -> SpelResult {
    AdminConfig::perform_transfer(&mut config, &caller, new_admin, &new_admin_account)?;
    Ok(SpelOutput::execute(
        vec![config.account, caller.account, new_admin_account.account],
        vec![],
    ))
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
    AdminConfig::perform_renounce(&mut config, &caller)?;
    Ok(SpelOutput::execute(
        vec![config.account, caller.account],
        vec![],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(id_byte: u8, signed: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account::default(),
            is_authorized: signed,
            account_id: AccountId::new([id_byte; 32]),
        }
    }

    #[test]
    fn initialize_sets_admin() {
        let cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        assert_eq!(cfg.slot.holder(), AccountId::new([1; 32]));
    }

    #[test]
    fn initialize_rejects_default_account_id() {
        assert_eq!(
            AdminConfig::initialize(AccountId::default()),
            Err(AdminError::InvalidCandidate)
        );
    }

    #[test]
    fn assert_admin_accepts_admin() {
        let cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        assert_eq!(cfg.assert_admin(&signer), Ok(()));
    }

    #[test]
    fn assert_admin_rejects_wrong_signer() {
        let cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(2, true);
        assert_eq!(cfg.assert_admin(&signer), Err(AdminError::NotAdmin))
    }

    #[test]
    fn assert_admin_rejects_unsigned() {
        let cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, false);
        assert_eq!(cfg.assert_admin(&signer), Err(AdminError::MissingSignature))
    }

    #[test]
    fn assert_admin_rejects_renounced() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        cfg.renounce(&signer).unwrap();
        assert_eq!(cfg.assert_admin(&signer), Err(AdminError::Renounced))
    }

    #[test]
    fn transfer_updates_admin() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        let new_admin = acct(2, true);
        cfg.transfer(&signer, AdminCandidate::Signer, &new_admin)
            .unwrap();
        assert_eq!(cfg.assert_admin(&new_admin), Ok(()));
    }

    #[test]
    fn transfer_rejects_non_admin_caller() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(2, true);
        let new_admin = acct(3, true);
        assert_eq!(
            cfg.transfer(&signer, AdminCandidate::Signer, &new_admin),
            Err(AdminError::NotAdmin)
        )
    }

    #[test]
    fn transfer_rejects_unsigned_candidate() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        let new_admin = acct(2, false);
        assert_eq!(
            cfg.transfer(&signer, AdminCandidate::Signer, &new_admin),
            Err(AdminError::InvalidCandidate)
        );
    }

    #[test]
    fn transfer_to_pda_validates_deployed() {
        let signer = acct(1, true);
        let mut cfg = AdminConfig::initialize(signer.account_id).unwrap();
        let (program_id, seed) = ([7; 8], [9; 32]);
        let pda_account = AccountWithMetadata {
            account: Account {
                program_owner: program_id,
                balance: 1,
                ..Account::default()
            }, // deployed: program-owned
            is_authorized: false,
            account_id: AccountId::for_public_pda(&program_id, &PdaSeed::new(seed)), // the REAL derived address
        };
        let candidate = AdminCandidate::Pda { program_id, seed };
        assert_eq!(cfg.transfer(&signer, candidate, &pda_account), Ok(()));
        assert_eq!(cfg.slot.holder(), pda_account.account_id);
    }

    #[test]
    fn transfer_rejects_pda_candidate_mismatch() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        let candidate = AdminCandidate::Pda {
            program_id: [7; 8],
            seed: [9; 32],
        };
        assert_eq!(
            cfg.transfer(&signer, candidate, &acct(3, false)),
            Err(AdminError::CandidateMismatch)
        );
    }

    #[test]
    fn transfer_rejects_undeployed_pda() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        let (program_id, seed) = ([7; 8], [9; 32]);
        let pda_account = AccountWithMetadata {
            account: Account::default(),
            is_authorized: false,
            account_id: AccountId::for_public_pda(&program_id, &PdaSeed::new(seed)),
        };
        assert_eq!(
            cfg.transfer(
                &signer,
                AdminCandidate::Pda { program_id, seed },
                &pda_account
            ),
            Err(AdminError::UndeployedPda)
        );
    }

    #[test]
    fn transfer_rejects_funded_but_unclaimed_pda() {
        // Anyone can send balance to the derived address. Without a program
        // claim, program_owner stays default and the candidate must be
        // rejected: a funded address is not a deployed PDA.
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        let (program_id, seed) = ([7; 8], [9; 32]);
        let pda_account = AccountWithMetadata {
            account: Account {
                balance: 1,
                ..Account::default()
            }, // funded, but no program owns it
            is_authorized: false,
            account_id: AccountId::for_public_pda(&program_id, &PdaSeed::new(seed)),
        };
        assert_eq!(
            cfg.transfer(
                &signer,
                AdminCandidate::Pda { program_id, seed },
                &pda_account
            ),
            Err(AdminError::UndeployedPda)
        );
        assert_eq!(cfg.slot.holder(), AccountId::new([1; 32]));
    }

    #[test]
    fn transfer_rejects_default_id_candidate() {
        // The default AccountId is the renounced sentinel. Installing it via
        // transfer would be a silent renounce, so validation rejects it.
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let current = acct(1, true);
        let candidate_account = acct(0, true); // default AccountId, co-signed
        assert_eq!(
            cfg.transfer(&current, AdminCandidate::Signer, &candidate_account),
            Err(AdminError::InvalidCandidate)
        );
        assert_eq!(cfg.slot.holder(), AccountId::new([1; 32]));
    }

    #[test]
    fn renounce_zeros_admin() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        cfg.renounce(&signer).unwrap();
        assert_eq!(cfg.slot.holder(), AccountId::default());
    }

    #[test]
    fn renounce_reject_non_admin() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(2, true);
        assert_eq!(cfg.renounce(&signer), Err(AdminError::NotAdmin));
    }

    #[test]
    fn renounce_is_permanent() {
        let mut cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let signer = acct(1, true);
        cfg.renounce(&signer).unwrap();
        assert_eq!(
            cfg.transfer(&signer, AdminCandidate::Signer, &signer),
            Err(AdminError::Renounced)
        )
    }

    #[test]
    fn encode_decode_roundtrip() {
        let cfg = AdminConfig::initialize(AccountId::new([1; 32])).unwrap();
        let encoded = cfg.encode().unwrap();
        let decoded = AdminConfig::decode(&encoded).unwrap();
        assert_eq!(cfg, decoded);
    }

    #[test]
    fn from_account_on_empty_data_returns_not_initialized() {
        let account = acct(1, true);
        assert_eq!(
            AdminConfig::from_account(&account),
            Err(AdminError::NotInitialized)
        )
    }

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
