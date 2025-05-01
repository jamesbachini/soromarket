#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::{Address as _}, Env, Address, String, symbol_short};
use sep_41_token::testutils::{MockTokenClient, MockTokenWASM};

fn create_token_contract(env: &Env) -> (Address, MockTokenClient) {
    let admin = Address::generate(&env);
    let token_id = env.register_contract_wasm(None, MockTokenWASM);
    let token_client = MockTokenClient::new(&env, &token_id);
    token_client.initialize(
        &admin,
        &7_u32,
        &String::from_str(&env, "Name"),
        &String::from_str(&env, "Symbol"),
    );
    (token_id, token_client)
}

fn setup(env: &Env) -> (
    SoroMarketClient,
    Address,
    Address,
    Address,
    Address,
) {
    let oracle = Address::generate(&env);
    let bettor1 = Address::generate(&env);
    let bettor2 = Address::generate(&env);
    let (token_id, token_client) = create_token_contract(&env);
    let contract_id = env.register_contract(None, SoroMarket);
    let client = SoroMarketClient::new(&env, &contract_id);
    env.mock_all_auths();
    let initial = 1_000_i128;
    token_client.mint(&bettor1, &initial);
    token_client.approve(&bettor1, &contract_id, &initial, &0_u32);
    token_client.mint(&bettor2, &initial);
    token_client.approve(&bettor2, &contract_id, &initial, &0_u32);
    client.setup(&oracle, &token_id, &String::from_str(&env, "James Will Be Next President Of The USA"));
    (client, oracle, token_id, bettor1, bettor2)
}

#[test]
fn test_setup_stores_values() {
    let env = Env::default();
    let (client, oracle, token, _, _) = setup(&env);
    env.as_contract(&client.address, || {
        let stored_oracle: Address = env.storage().persistent().get(&symbol_short!("oracle")).unwrap();
        let stored_token: Address = env.storage().persistent().get(&symbol_short!("token")).unwrap();
        let state: Outcome = env.storage().persistent().get(&symbol_short!("state")).unwrap();
        assert_eq!(stored_oracle, oracle);
        assert_eq!(stored_token, token);
        assert_eq!(state, Outcome::Undecided);
    });
}

#[test]
fn test_bet_and_totals() {
    let env = Env::default();
    let (client, _, _token, bettor1, bettor2) = setup(&env);
    client.bet(&bettor1, &100, &true);
    client.bet(&bettor2, &200, &false);
    env.as_contract(&client.address, || {
        let tt: i128 = env.storage().persistent().get(&symbol_short!("truetotal")).unwrap();
        let ft: i128 = env.storage().persistent().get(&symbol_short!("falsetot")).unwrap();
        assert_eq!(tt, 100);
        assert_eq!(ft, 200);
        let bets: Map<Address, Bets> = env.storage().persistent().get(&symbol_short!("bets")).unwrap();
        let b1 = bets.get(bettor1.clone()).unwrap();
        assert_eq!(b1.amount, 100);
        assert!(b1.bet_on_true);
        assert!(!b1.claimed);
    });
}

#[test]
#[should_panic(expected = "Already bet")]
fn test_double_bet_panics() {
    let env = Env::default();
    let (client, _, _, bettor1, _) = setup(&env);
    client.bet(&bettor1, &50, &true);
    client.bet(&bettor1, &30, &false);
}

#[test]
#[should_panic(expected = "Market not live")]
fn test_bet_after_settle_panics() {
    let env = Env::default();
    let (client, oracle, _, bettor1, _) = setup(&env);
    client.settle(&oracle, &true);
    client.bet(&bettor1, &10, &true);
}

#[test]
fn test_claim_winners_and_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2) = setup(&env);
    client.bet(&bettor1, &100, &true);
    client.bet(&bettor2, &100, &false);
    client.settle(&oracle, &true);
    let mock_token = MockTokenClient::new(&env, &token_id);
    let before1 = mock_token.balance(&bettor1);
    let before2 = mock_token.balance(&bettor2);
    client.claim(&bettor1);
    client.claim(&bettor2);
    let after1 = mock_token.balance(&bettor1);
    let after2 = mock_token.balance(&bettor2);
    assert!(after1 > before1);
    assert_eq!(after2, before2);
}

#[test]
#[should_panic(expected = "Already claimed")]
fn test_double_claim_panics() {
    let env = Env::default();
    let (client, oracle, _, bettor1, _) = setup(&env);
    client.bet(&bettor1, &100, &true);
    client.settle(&oracle, &true);
    client.claim(&bettor1);
    client.claim(&bettor1);
}
