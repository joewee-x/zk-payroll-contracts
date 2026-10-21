//! Contract-level payout ordering validation (#815).
//!
//! Covers the public entrypoints `commit_recipient_ordering` /
//! `check_payout_ordering` and the opt-in guard on `prepare_payroll_run`:
//! success when the payout matches, rejection when order differs, and the
//! single-employee boundary.

#![cfg(test)]

mod common;

use payroll::{PayoutOrderingCheck, MAX_ORDERING_SIZE};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, Vec};

fn mock_proof(env: &Env) -> BytesN<256> {
    BytesN::from_array(env, &[1; 256])
}

fn prepare_args(env: &Env, employees: &Vec<Address>) -> (Vec<BytesN<256>>, Vec<i128>) {
    let mut proofs = Vec::new(env);
    let mut amounts = Vec::new(env);
    for _ in 0..employees.len() {
        proofs.push_back(mock_proof(env));
        amounts.push_back(100_i128);
    }
    (proofs, amounts)
}

#[test]
fn commit_and_matching_prepare_succeeds() {
    let env = Env::default();
    let (client, _token, employee) = common::setup(&env);
    let admin = client.get_addresses().admin;

    // Store a second commitment so we can exercise a two-recipient order.
    let commitment_addr = client.get_addresses().commitment;
    let commitment = salary_commitment::SalaryCommitmentContractClient::new(&env, &commitment_addr);
    let bob = Address::generate(&env);
    commitment.store_commitment(&bob, &BytesN::from_array(&env, &[2; 32]));

    let employees = Vec::from_array(&env, [employee.clone(), bob.clone()]);
    let nonce = common::nonce(&env, 81);
    client.commit_recipient_ordering(&admin, &nonce, &employees);

    let check = client.check_payout_ordering(&nonce, &employees);
    assert!(check.all_passed());

    let (proofs, amounts) = prepare_args(&env, &employees);
    let run_id = client.prepare_payroll_run(&proofs, &amounts, &employees, &200, &nonce, &None);
    assert!(run_id > 0);

    // Commitment is consumed by successful prepare.
    let after: PayoutOrderingCheck = client.check_payout_ordering(&nonce, &employees);
    assert!(!after.commitment_found);
}

#[test]
fn prepare_rejects_wrong_recipient_order() {
    let env = Env::default();
    let (client, _token, employee) = common::setup(&env);
    let admin = client.get_addresses().admin;

    let commitment_addr = client.get_addresses().commitment;
    let commitment = salary_commitment::SalaryCommitmentContractClient::new(&env, &commitment_addr);
    let bob = Address::generate(&env);
    commitment.store_commitment(&bob, &BytesN::from_array(&env, &[2; 32]));

    let committed = Vec::from_array(&env, [employee.clone(), bob.clone()]);
    let reversed = Vec::from_array(&env, [bob.clone(), employee.clone()]);
    let nonce = common::nonce(&env, 82);
    client.commit_recipient_ordering(&admin, &nonce, &committed);

    let check = client.check_payout_ordering(&nonce, &reversed);
    assert!(check.commitment_found);
    assert!(check.length_matches);
    assert!(!check.order_matches);

    let (proofs, amounts) = prepare_args(&env, &reversed);
    let result = client.try_prepare_payroll_run(&proofs, &amounts, &reversed, &200, &nonce, &None);
    assert!(result.is_err());

    // Failure must not consume the commitment.
    let still = client.check_payout_ordering(&nonce, &committed);
    assert!(still.all_passed());
}

#[test]
fn single_employee_boundary_succeeds() {
    let env = Env::default();
    let (client, _token, employee) = common::setup(&env);
    let admin = client.get_addresses().admin;

    let employees = Vec::from_array(&env, [employee.clone()]);
    let nonce = common::nonce(&env, 83);
    client.commit_recipient_ordering(&admin, &nonce, &employees);

    let (proofs, amounts) = prepare_args(&env, &employees);
    let run_id = client.prepare_payroll_run(&proofs, &amounts, &employees, &100, &nonce, &None);
    assert!(run_id > 0);
}

#[test]
fn opt_in_prepare_without_commitment_still_works() {
    let env = Env::default();
    let (client, _token, employee) = common::setup(&env);
    let employees = Vec::from_array(&env, [employee.clone()]);
    let nonce = common::nonce(&env, 84);
    let (proofs, amounts) = prepare_args(&env, &employees);
    let run_id = client.prepare_payroll_run(&proofs, &amounts, &employees, &100, &nonce, &None);
    assert!(run_id > 0);
}

#[test]
fn commit_rejects_empty_and_oversized_lists() {
    let env = Env::default();
    let (client, _token, _employee) = common::setup(&env);
    let admin = client.get_addresses().admin;

    let empty: Vec<Address> = Vec::new(&env);
    assert!(client
        .try_commit_recipient_ordering(&admin, &common::nonce(&env, 85), &empty)
        .is_err());

    let mut oversized = Vec::new(&env);
    for _ in 0..=MAX_ORDERING_SIZE {
        oversized.push_back(Address::generate(&env));
    }
    assert!(client
        .try_commit_recipient_ordering(&admin, &common::nonce(&env, 86), &oversized)
        .is_err());
}

#[test]
fn non_admin_cannot_commit_ordering() {
    let env = Env::default();
    let (client, _token, employee) = common::setup(&env);
    let impostor = Address::generate(&env);
    let recipients = Vec::from_array(&env, [employee.clone()]);
    assert!(client
        .try_commit_recipient_ordering(&impostor, &common::nonce(&env, 87), &recipients)
        .is_err());
}
