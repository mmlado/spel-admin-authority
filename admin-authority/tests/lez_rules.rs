//! The library's own outputs run through LEZ's `validate_execution`, then
//! claims and nonce bumps applied the way a sequencer applies them.

use admin_authority::{
    AdminCandidate, AdminError, admin_accept, admin_initialize, admin_nominate, admin_renounce,
    admin_transfer,
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
fn admin_initialize_applies_the_genesis_key() {
    // CI runs this file again with SPEL_ADMIN_GENESIS set to 01 x 32.
    let result = admin_initialize(config(), owned(2));
    match admin_authority::genesis_admin() {
        Some(_) => match result {
            Err(err) => assert_eq!(
                err.to_string(),
                SpelError::from(AdminError::NotGenesisAdmin).to_string()
            ),
            Ok(_) => panic!("a key other than the genesis key became admin"),
        },
        None => assert!(result.is_ok()),
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

const HOLDER_PROGRAM: ProgramId = [9; 8];

/// Config with owned key `1` as admin. Returns `[config, admin]`.
fn initialised() -> (AccountWithMetadata, AccountWithMetadata) {
    let admin = owned(1);
    let after = land(
        vec![config(), admin.clone()],
        posts(admin_initialize(config(), admin)),
    )
    .unwrap();
    (after[0].clone(), signing(after[1].clone()))
}

fn nominate(
    config: &AccountWithMetadata,
    admin: &AccountWithMetadata,
    nominee: &AccountWithMetadata,
) -> AccountWithMetadata {
    let pre = vec![config.clone(), admin.clone()];
    let after = land(
        pre,
        posts(admin_nominate(
            config.clone(),
            admin.clone(),
            *nominee.account_id.value(),
            0,
        )),
    )
    .unwrap();
    after[0].clone()
}

#[test]
fn a_nominee_accepts_and_then_administers() {
    let (config, admin) = initialised();
    let nominee = owned(2);
    let config = nominate(&config, &admin, &nominee);

    let accepted = land(
        vec![config.clone(), nominee.clone()],
        posts(admin_accept(config, nominee, 0)),
    )
    .unwrap();

    let renounce = posts(admin_renounce(
        accepted[0].clone(),
        signing(accepted[1].clone()),
        0,
    ));
    land(
        vec![accepted[0].clone(), signing(accepted[1].clone())],
        renounce,
    )
    .expect("LEZ accepts the new admin's next transaction");
}

#[test]
fn a_never_used_nominee_is_refused_at_accept() {
    let (config, admin) = initialised();
    let nominee = never_used(2);
    let config = nominate(&config, &admin, &nominee);
    refused_as_unowned(admin_accept(config, nominee, 0));
}

#[test]
fn a_stale_nominee_cannot_accept_after_a_transfer() {
    let (config, admin) = initialised();
    let stale = owned(2);
    let config = nominate(&config, &admin, &stale);

    let next = owned(3);
    let pre = vec![config.clone(), admin.clone(), next.clone()];
    let after = land(
        pre,
        posts(admin_transfer(
            config,
            admin,
            next,
            AdminCandidate::Signer,
            0,
        )),
    )
    .unwrap();

    let expected = SpelError::from(AdminError::NoNomination);
    match admin_accept(after[0].clone(), stale, 0) {
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
        Ok(_) => panic!("a stale nominee took the admin after a transfer"),
    }
}

#[test]
fn nothing_can_accept_after_renounce() {
    let (config, admin) = initialised();
    let nominee = owned(2);
    let config = nominate(&config, &admin, &nominee);
    let after = land(
        vec![config.clone(), admin.clone()],
        posts(admin_renounce(config, admin, 0)),
    )
    .unwrap();
    assert!(admin_accept(after[0].clone(), nominee, 0).is_err());
}

#[test]
fn a_pda_nominee_accepts_through_its_owning_program() {
    // A chained call with the PDA's seeds reaches the library as a signed,
    // program-owned account.
    let (config, admin) = initialised();
    let pda = AccountWithMetadata {
        account: Account {
            program_owner: HOLDER_PROGRAM,
            data: vec![1].try_into().unwrap(),
            ..Account::default()
        },
        is_authorized: true,
        account_id: AccountId::for_public_pda(
            &HOLDER_PROGRAM,
            &PdaSeed::new(seed_from_str("vault")),
        ),
    };
    let config = nominate(&config, &admin, &pda);
    let accepted = land(
        vec![config.clone(), pda.clone()],
        posts(admin_accept(config, pda.clone(), 0)),
    )
    .unwrap();

    let renounce = posts(admin_renounce(accepted[0].clone(), pda.clone(), 0));
    land(vec![accepted[0].clone(), pda], renounce).expect("the PDA administers");
}
