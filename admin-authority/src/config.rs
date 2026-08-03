#![warn(missing_docs)]

use authority::{AuthorityCandidate, AuthoritySlot};
use borsh::{BorshDeserialize, BorshSerialize};
use spel_framework::prelude::*;

use crate::errors::*;

/// Transfer-time claim describing the intended new admin. Alias of the
/// shared [`AuthorityCandidate`]: `Signer` proves key control by
/// co-signature, `Pda { program_id, seed }` by address derivation plus a
/// deployment check. Always paired with a `new_account` param that
/// carries the chain-state evidence.
pub type AdminCandidate = AuthorityCandidate;

/// Borsh-encoded size of the config, the embedded window width.
pub const ENCODED_LEN: usize = AuthoritySlot::ENCODED_LEN;

/// On-chain admin authority state for a single program.
///
/// Stored in the program's Config PDA at `(program_id, "admin_config")`.
/// Created once via `admin_initialize`; cannot be reinitialized. Wraps the
/// shared [`AuthoritySlot`], whose holder is the current admin and whose
/// `AccountId::default()` sentinel marks the renounced state. The borsh
/// layout is a single 32-byte `AccountId`, unchanged by the extraction.
#[account_type]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AdminConfig {
    slot: AuthoritySlot,
}

impl AdminConfig {
    /// Constructs an initialised AdminConfig.
    ///
    /// Rejects `AccountId::default()` as the admin since that value is the
    /// reserved sentinel for the renounced state.
    pub fn initialize(admin: AccountId) -> Result<Self, AdminError> {
        Ok(Self {
            slot: AuthoritySlot::initialize(admin)?,
        })
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

    /// Strict decode of the config's window at `offset`.
    ///
    /// Keeps the three-way discrimination: empty data means the
    /// embedding account does not exist yet (`NotInitialized`);
    /// non-empty data too short for the window is `SlotOutOfBounds`,
    /// a layout error. Dedicated mode is the degenerate case
    /// `offset = 0` over the Config PDA.
    ///
    /// # Errors
    ///
    /// `AdminError::NotInitialized` on empty data,
    /// `AdminError::SlotOutOfBounds` when the window does not fit.
    pub fn decode_at(data: &[u8], offset: usize) -> Result<Self, AdminError> {
        if data.is_empty() {
            return Err(AdminError::NotInitialized);
        }
        Ok(Self {
            slot: AuthoritySlot::read_at(data, offset)?,
        })
    }

    /// Loads config from an account's data field. Convenience wrapper over
    /// [`AdminConfig::decode`].
    pub fn from_account(account: &AccountWithMetadata) -> Result<Self, AdminError> {
        Self::decode(&account.account.data)
    }

    /// Loads the config from an account's data at `offset`. Convenience
    /// wrapper over [`AdminConfig::decode_at`].
    pub fn from_account_at(
        account: &AccountWithMetadata,
        offset: usize,
    ) -> Result<Self, AdminError> {
        Self::decode_at(&account.account.data, offset)
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

    /// Splices only the config's window at `offset` into the account's
    /// data, leaving every surrounding byte untouched.
    ///
    /// # Errors
    ///
    /// `AdminError:SlotOutOfBounds` when the window does not fit.
    pub fn write_to_at(
        &self,
        account: &mut AccountWithMetadata,
        offset: usize,
    ) -> Result<(), AdminError> {
        let mut bytes: Vec<u8> = account.account.data.to_vec();
        self.slot.write_at(&mut bytes, offset)?;
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
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let resolved = candidate.validate(new_account)?;
        let state = Self::initialize(resolved)?;
        state.write_to(config_account)
    }

    /// Validates a candidate, builds a fresh config, and splices it in
    /// at `offset`. The embedded-mode bootstrap: the consumer's
    /// account-creating instruction calls this after writing its own
    /// state, so the slot is born initialized.
    ///
    /// # Errors
    ///
    /// Candidate validation errors, plus `SlotOufOfBounds` when the
    /// account's data does not cover the window (write the full
    /// consumer struct before bootstrapping).
    pub fn bootstrap_at(
        config_account: &mut AccountWithMetadata,
        offset: usize,
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let resolved = candidate.validate(new_account)?;
        let state = Self::initialize(resolved)?;
        state.write_to_at(config_account, offset)
    }

    /// Loads config from account, transfers admin, writes back.
    pub fn perform_transfer(
        config_account: &mut AccountWithMetadata,
        current: &AccountWithMetadata,
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        Self::perform_transfer_at(config_account, 0, current, candidate, new_account)
    }

    /// Loads config from account, transfers admin, writes back.
    pub fn perform_transfer_at(
        config_account: &mut AccountWithMetadata,
        offset: usize,
        current: &AccountWithMetadata,
        candidate: AdminCandidate,
        new_account: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let mut state = Self::from_account_at(config_account, offset)?;
        state.transfer(current, candidate, new_account)?;
        state.write_to_at(config_account, offset)
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

    /// Loads config from account, renounce admin, writes back.
    pub fn perform_renounce_at(
        config_account: &mut AccountWithMetadata,
        offset: usize,
        current: &AccountWithMetadata,
    ) -> Result<(), AdminError> {
        let mut state = Self::from_account_at(config_account, offset)?;
        state.renounce(current)?;
        state.write_to_at(config_account, offset)
    }
}

impl spel_framework::FixedBorshSize for AdminConfig {
    const SIZE: usize = 32;
}

impl spel_framework::SlotLayoutProbe for AdminConfig {
    fn probe() -> Self {
        AdminConfig::initialize(AccountId::new([0xA5; 32]))
            .expect("the probe admin is is not the renounced sentinel")
    }
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

    // An account whose data is `len` bytes of 0xAA sentinel.
    fn sentinel_acct(len: usize) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account {
                data: vec![0xAA; len].try_into().unwrap(),
                ..Account::default()
            },
            is_authorized: false,
            account_id: AccountId::new([9; 32]),
        }
    }

    #[test]
    fn perform_transfer_at_preserves_neighbors_and_installs_new_admin() {
        let mut config_account = sentinel_acct(72);
        let old_admin = acct(1, true);
        let new_admin = acct(2, true);
        AdminConfig::bootstrap_at(&mut config_account, 8, AdminCandidate::Signer, &old_admin)
            .expect("bootstrap must succeed");

        AdminConfig::perform_transfer_at(
            &mut config_account,
            8,
            &old_admin,
            AdminCandidate::Signer,
            &new_admin,
        )
        .expect("transfer at offset 8 must succeed");

        let data = &config_account.account.data;
        assert!(data[..8].iter().all(|b| *b == 0xAA), "prefix trampled");
        assert!(data[40..].iter().all(|b| *b == 0xAA), "suffix trampled");
        let cfg = AdminConfig::from_account_at(&config_account, 8).unwrap();
        assert!(cfg.assert_admin(&new_admin).is_ok());
        assert_eq!(cfg.assert_admin(&old_admin), Err(AdminError::NotAdmin));
    }

    #[test]
    fn perform_renounce_at_preserves_neighbors_and_is_terminal() {
        let mut config_account = sentinel_acct(72);
        let admin = acct(1, true);
        AdminConfig::bootstrap_at(&mut config_account, 8, AdminCandidate::Signer, &admin)
            .expect("bootstrap must succeed");

        AdminConfig::perform_renounce_at(&mut config_account, 8, &admin)
            .expect("renounce at offset 8 must succeed");

        let data = &config_account.account.data;
        assert!(data[..8].iter().all(|b| *b == 0xAA), "prefix trampled");
        assert!(data[40..].iter().all(|b| *b == 0xAA), "suffix trampled");
        let cfg = AdminConfig::from_account_at(&config_account, 8).unwrap();
        assert_eq!(cfg.assert_admin(&admin), Err(AdminError::Renounced));
    }

    #[test]
    fn dedicated_perform_transfer_delegates_to_offset_zero() {
        let mut config_account = acct(9, false);
        let old_admin = acct(1, true);
        let new_admin = acct(2, true);
        AdminConfig::bootstrap(&mut config_account, AdminCandidate::Signer, &old_admin)
            .expect("dedicated bootstrap must succeed");
        AdminConfig::perform_transfer(
            &mut config_account,
            &old_admin,
            AdminCandidate::Signer,
            &new_admin,
        )
        .expect("dedicated transfer must succeed");
        let cfg = AdminConfig::from_account(&config_account).unwrap();
        assert!(cfg.assert_admin(&new_admin).is_ok());
    }

    #[test]
    fn bootstrap_at_splices_at_offset_preserving_neighbors() {
        let mut config_account = sentinel_acct(72);
        let signer = acct(1, true);
        AdminConfig::bootstrap_at(&mut config_account, 8, AdminCandidate::Signer, &signer)
            .expect("bootstrap at offset 8 must succeed");
        let data = &config_account.account.data;
        assert!(data[..8].iter().all(|b| *b == 0xAA), "prefix trampled");
        assert!(data[40..].iter().all(|b| *b == 0xAA), "suffix trampled");
        let cfg = AdminConfig::from_account_at(&config_account, 8).unwrap();
        assert!(cfg.assert_admin(&signer).is_ok());
    }

    #[test]
    fn decode_at_empty_data_is_not_initialized() {
        assert_eq!(
            AdminConfig::decode_at(&[], 8),
            Err(AdminError::NotInitialized)
        );
    }

    #[test]
    fn decode_at_short_data_is_slot_out_of_bounds() {
        let data = [0u8; 20];
        assert_eq!(
            AdminConfig::decode_at(&data, 0),
            Err(AdminError::SlotOutOfBounds)
        );
    }

    #[test]
    fn bootstrap_at_on_empty_account_is_slot_out_of_bounds() {
        // The consumer forgot to write their struct before bootstrapping:
        // empty data cannot cover the window, loud error instead of a
        // silent resize.
        let mut config_account = acct(7, false);
        let signer = acct(1, true);
        assert_eq!(
            AdminConfig::bootstrap_at(&mut config_account, 32, AdminCandidate::Signer, &signer),
            Err(AdminError::SlotOutOfBounds)
        );
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
}
