//! The library's own outputs run through LEZ's `validate_execution`, then
//! claims and nonce bumps applied the way a sequencer applies them.

use admin_authority::{
    AdminCandidate, AdminError, admin_initialize, admin_renounce, admin_transfer,
};
use nssa_core::account::{Account, AccountId, AccountWithMetadata};
use nssa_core::program::{
    AccountPostState, ExecutionValidationError, PdaSeed, ProgramId, validate_execution,
};
use spel_framework::pda::seed_from_str;
use spel_framework::prelude::{SpelError, SpelResult};

const PROGRAM: ProgramId = [7; 8];
const WALLET_PROGRAM: ProgramId = [5; 8];

fn config() -> AccountWithMetadata {
    AccountWithMetadata {
        account: Account::default(),
        is_authorized: false,
        account_id: AccountId::for_public_pda(
            &PROGRAM,
            &PdaSeed::new(seed_from_str("admin_config")),
        ),
    }
}

fn never_used(id: u8) -> AccountWithMetadata {
    AccountWithMetadata {
        account: Account::default(),
        is_authorized: true,
        account_id: AccountId::new([id; 32]),
    }
}

fn owned(id: u8) -> AccountWithMetadata {
    let mut key = never_used(id);
    key.account.program_owner = WALLET_PROGRAM;
    key
}

fn signing(mut key: AccountWithMetadata) -> AccountWithMetadata {
    key.is_authorized = true;
    key
}

/// One transaction: LEZ's checks, then claims, then every signer's nonce bump.
fn land(
    pre: Vec<AccountWithMetadata>,
    posts: Vec<AccountPostState>,
) -> Result<Vec<AccountWithMetadata>, ExecutionValidationError> {
    validate_execution(&pre, &posts, PROGRAM)?;
    Ok(pre
        .into_iter()
        .zip(posts)
        .map(|(mut before, post)| {
            let claimed = post.required_claim().is_some();
            before.account = post.account().clone();
            if claimed {
                before.account.program_owner = PROGRAM;
            }
            if before.is_authorized {
                before.account.nonce.public_account_nonce_increment();
            }
            before
        })
        .collect())
}

fn posts(result: SpelResult) -> Vec<AccountPostState> {
    result
        .expect("the library accepts")
        .into_parts()
        .post_states
}

fn refused_as_unowned(result: SpelResult) {
    let expected = SpelError::from(AdminError::UnownedSigner);
    match result {
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
        Ok(_) => panic!("the library accepted a key no program owns"),
    }
}

#[test]
fn a_never_used_key_is_refused_as_the_first_admin() {
    refused_as_unowned(admin_initialize(config(), never_used(1)));
}

#[test]
fn a_never_used_key_is_refused_as_a_transfer_target() {
    let admin = owned(1);
    let pre = vec![config(), admin.clone()];
    let after = land(pre, posts(admin_initialize(config(), admin))).unwrap();

    refused_as_unowned(admin_transfer(
        after[0].clone(),
        signing(after[1].clone()),
        never_used(2),
        AdminCandidate::Signer,
        0,
    ));
}

#[test]
fn lez_rejects_an_unowned_admin_after_its_first_transaction() {
    // What the refusal prevents. On chain SPEL drops this account first and
    // LEZ answers DeclaredAccountMissingFromOutput instead.
    let mut admin = owned(1);
    let after = land(
        vec![config(), admin.clone()],
        posts(admin_initialize(config(), admin.clone())),
    )
    .unwrap();
    admin.account.program_owner = ProgramId::default();
    admin.account.nonce = after[1].account.nonce;

    let pre = vec![after[0].clone(), admin.clone()];
    let renounce = posts(admin_renounce(after[0].clone(), admin, 0));
    assert!(matches!(
        validate_execution(&pre, &renounce, PROGRAM),
        Err(ExecutionValidationError::NonDefaultAccountWithDefaultOwner { .. })
    ));
}

#[test]
fn an_owned_admin_keeps_working_after_initialize() {
    let admin = owned(1);
    let after = land(
        vec![config(), admin.clone()],
        posts(admin_initialize(config(), admin)),
    )
    .unwrap();

    let renounce = posts(admin_renounce(
        after[0].clone(),
        signing(after[1].clone()),
        0,
    ));
    land(after, renounce).expect("LEZ accepts the admin's second transaction");
}

#[test]
fn an_owned_transfer_target_keeps_working_as_admin() {
    let admin = owned(1);
    let after = land(
        vec![config(), admin.clone()],
        posts(admin_initialize(config(), admin)),
    )
    .unwrap();

    let next = owned(2);
    let pre = vec![after[0].clone(), signing(after[1].clone()), next.clone()];
    let transferred = land(
        pre,
        posts(admin_transfer(
            after[0].clone(),
            signing(after[1].clone()),
            next,
            AdminCandidate::Signer,
            0,
        )),
    )
    .unwrap();

    let pre = vec![transferred[0].clone(), signing(transferred[2].clone())];
    let renounce = posts(admin_renounce(
        transferred[0].clone(),
        signing(transferred[2].clone()),
        0,
    ));
    land(pre, renounce).expect("LEZ accepts the new admin's next transaction");
}
