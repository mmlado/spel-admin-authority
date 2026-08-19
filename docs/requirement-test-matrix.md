# Requirement-to-test matrix, RFP-001 (M3)

Every hard requirement and every proposal-enumerated scenario, with the artifact that satisfies it. Test names are exact. Regenerate the coverage claim by running `RISC0_DEV_MODE=1 cargo test --workspace`.

## Hard requirements, Functionality

| # | Requirement (RFP-001) | Satisfied by | Kind |
| --- | --- | --- | --- |
| F1 | Admin authority is set at program initialisation | `initialize_sets_admin`, `bootstrap_at_splices_at_offset_preserving_neighbors` (embedded), self-election design in ADR-0005 | test |
| F2 | Admin can transfer to a new signer | `transfer_updates_admin`, `admin_transfer_updates_admin`, `perform_transfer_at_preserves_neighbors_and_installs_new_admin` | test |
| F3 | Admin can revoke, renouncing control | `renounce_zeros_admin`, `renounce_is_permanent`, `perform_renounce_at_preserves_neighbors_and_is_terminal` | test |
| F4 | Only admin calls privileged instructions (gated config PDA update) | `update_value_succeeds_for_admin`, `update_value_rejects_non_admin` (exact error message), `update_value_rejects_after_renounce_`, `admin_renounce_then_transfer_fails` | test |

## Hard requirements, Usability

| # | Requirement | Satisfied by | Kind |
| --- | --- | --- | --- |
| U1 | SPEL integration, minimal boilerplate | `#[admin_authority]` + `#[require_admin]`, three reference samples, `idl_contains_user_instr_and_admin_trio`; framework side `inject_gate_params_injects_missing_and_skips_declared`, `role_matched_params_skip_injection` | sample + test |
| U2 | One admin at a time | Single `AccountId` slot in `AdminConfig`, `transfer_updates_admin` asserts the old admin loses access | design + test |
| U3 | End-to-end usage example in docs | README integration steps, `scripts/dry-run.sh` with committed output | doc |

## Hard requirements, Performance

| # | Requirement | Satisfied by | Kind |
| --- | --- | --- | --- |
| P1 | Document transaction size overhead of the gate | README overhead section, measured from the committed dry-run captures | doc |

## Hard requirements, Supportability

| # | Requirement | Satisfied by | Kind |
| --- | --- | --- | --- |
| S1 | CI green on default branch | `.github/workflows/ci.yml` | ci |
| S2 | Every hard requirement has a test | this matrix | doc |
| S3 | README documents dependency and integration steps | README, Adding as a dependency + Integration steps | doc |
| S4 | Sample program included | `admin-authority-sample`, `admin-authority-sample-manual`, `admin-authority-sample-embedded` | sample |

## Soft requirement, Reliability

| # | Requirement | Satisfied by | Kind |
| --- | --- | --- | --- |
| R1 | Admin set only to a valid new signer (on-curve key or deployed PDA) | `transfer_rejects_unsigned_candidate`, `transfer_rejects_default_id_candidate`, `transfer_rejects_undeployed_pda`, `transfer_rejects_funded_but_unclaimed_pda`, `transfer_rejects_pda_candidate_mismatch`, `transfer_to_pda_validates_deployed`, `initialize_rejects_default_account_id` | test |

## Proposal scenarios (logos-co/rfp#46)

| Scenario | Satisfied by |
| --- | --- |
| Initialization | `initialize_sets_admin` |
| Re-initialization rejection | `admin_initialize_rejects_already_initialized_config` (framework `#[account(init)]` validation). Embedded mode reinit rides the consumer account's init |
| Admin gated-call success | `update_value_succeeds_for_admin` (both samples) |
| Non-admin gated-call rejection | `update_value_rejects_non_admin` (exact message asserted) |
| Transfer success | `transfer_updates_admin`, `admin_transfer_updates_admin` |
| Non-admin transfer rejection | `admin_transfer_rejects_non_admin`, `transfer_rejects_non_admin_caller` |
| Renounce success | `renounce_zeros_admin` |
| Protected instructions rejected after renounce | `update_value_rejects_after_renounce_`, `admin_renounce_then_transfer_fails`, `assert_admin_rejects_renounced` |

## M2.5 embedded surface

Delivered with the m2_5 branch set, evidence: the M2.5 PR set, `docs/dry-run-embedded-output.txt`, and:

| Behavior | Satisfied by |
| --- | --- |
| Slot embedded at marker offset, neighbors preserved | `bootstrap_at_splices_at_offset_preserving_neighbors`, `perform_transfer_at_preserves_neighbors_and_installs_new_admin`, `value_survives_admin_transfer` |
| Born initialized, no `admin_initialize` emitted | `idl_shows_embedded_surface_and_no_initializer_or_offset`; framework side `embedded_mode_skips_declared_initializer` |
| Skipped bootstrap is born renounced | `skipped_bootstrap_is_born_renounced` |
| Role substitution to the consumer account | framework `substituted_role_param_takes_consumer_name_and_constraint`, `canonical_constraint_resolves_from_init_declaration` |
| Offsets never in IDL or transaction | `idl_shows_embedded_surface_and_no_initializer_or_offset`, dry-run capture; framework side `consumer_offset_kwarg_on_embedded_gate_is_error`, `wrap_stamped_attr_carries_embedded_offset` |
| Dedicated mode unchanged | dedicated dry-run byte-identical to the M2 pin |

Every row above names at least one passing test. No known gaps.
