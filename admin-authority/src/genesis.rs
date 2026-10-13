//! The genesis key: which account may run `admin_initialize`, compiled in
//! from `SPEL_ADMIN_GENESIS` at the consumer's build.

use spel_framework::prelude::*;

use crate::errors::AdminError;

/// The genesis key, or `None` when the build sets no `SPEL_ADMIN_GENESIS`.
/// A malformed value fails the build.
const GENESIS: Option<[u8; 32]> = match option_env!("SPEL_ADMIN_GENESIS") {
    Some(hex) => Some(genesis_from_hex(hex)),
    None => None,
};

/// The account this build lets run `admin_initialize`, if it carries one.
pub fn genesis_admin() -> Option<AccountId> {
    GENESIS.map(AccountId::new)
}

/// Refuses `caller` when this build carries a genesis key that is not it.
/// Without one, anyone passes.
///
/// `admin_initialize` calls this. Embedded consumers can call it in their
/// own account-creating instruction to gate it the same way.
pub fn require_genesis(caller: &AccountWithMetadata) -> Result<(), AdminError> {
    check_genesis(GENESIS, caller)
}

fn check_genesis(
    genesis: Option<[u8; 32]>,
    caller: &AccountWithMetadata,
) -> Result<(), AdminError> {
    match genesis {
        Some(key) if caller.account_id != AccountId::new(key) => Err(AdminError::NotGenesisAdmin),
        _ => Ok(()),
    }
}

const fn genesis_from_hex(hex: &str) -> [u8; 32] {
    let bytes = hex.as_bytes();
    assert!(
        bytes.len() == 64,
        "SPEL_ADMIN_GENESIS must be 64 hexadecimal characters"
    );
    let mut key = [0u8; 32];
    let mut nonzero = false;
    let mut i = 0;
    while i < 32 {
        key[i] = (nibble(bytes[2 * i]) << 4) | nibble(bytes[2 * i + 1]);
        nonzero |= key[i] != 0;
        i += 1;
    }
    // The all-zero id is the renounced sentinel, nobody can sign as it.
    assert!(nonzero, "SPEL_ADMIN_GENESIS must not be all zeros");
    key
}

const fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("SPEL_ADMIN_GENESIS must be hexadecimal"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer(id: u8) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account::default(),
            is_authorized: true,
            account_id: AccountId::new([id; 32]),
        }
    }

    #[test]
    fn parses_a_64_character_hex_key() {
        let hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let key = genesis_from_hex(hex);
        assert_eq!(key[0], 1);
        assert_eq!(key[31], 0x20);
        assert_eq!(genesis_from_hex(&hex.to_uppercase()), key);
    }

    #[test]
    #[should_panic(expected = "64 hexadecimal characters")]
    fn a_short_key_is_refused() {
        genesis_from_hex("0102");
    }

    #[test]
    #[should_panic(expected = "must be hexadecimal")]
    fn a_non_hex_key_is_refused() {
        genesis_from_hex(&"g".repeat(64));
    }

    #[test]
    #[should_panic(expected = "must not be all zeros")]
    fn the_all_zero_key_is_refused() {
        genesis_from_hex(&"0".repeat(64));
    }

    #[test]
    fn without_a_genesis_key_anyone_may_initialize() {
        assert_eq!(check_genesis(None, &signer(3)), Ok(()));
    }

    #[test]
    fn with_a_genesis_key_only_that_key_may_initialize() {
        assert_eq!(check_genesis(Some([1; 32]), &signer(1)), Ok(()));
        assert_eq!(
            check_genesis(Some([1; 32]), &signer(3)),
            Err(AdminError::NotGenesisAdmin)
        );
    }
}
