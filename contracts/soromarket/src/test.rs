#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::{Address as _}, Env, Address, String};
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
    Address,
) {
    let oracle = Address::generate(&env);
    let deployer = Address::generate(&env);
    let bettor1 = Address::generate(&env);
    let bettor2 = Address::generate(&env);
    let (token_id, token_client) = create_token_contract(&env);
    let contract_id = env.register_contract(None, SoroMarket);
    let client = SoroMarketClient::new(&env, &contract_id);
    env.mock_all_auths();
    let initial = 1_000_i128;
    let initial_liquidity = 2000_i128; // 1000 for each side
    token_client.mint(&deployer, &initial_liquidity);
    token_client.approve(&deployer, &contract_id, &initial_liquidity, &0_u32);
    token_client.mint(&bettor1, &initial);
    token_client.approve(&bettor1, &contract_id, &initial, &0_u32);
    token_client.mint(&bettor2, &initial);
    token_client.approve(&bettor2, &contract_id, &initial, &0_u32);
    
    client.setup(&deployer, &oracle, &token_id, &String::from_str(&env, "James Will Be Next President Of The USA"), &1000);
    (client, oracle, token_id, bettor1, bettor2, deployer)
}

#[test]
fn test_setup_stores_values() {
    let env = Env::default();
    let (client, oracle, token, _, _, _) = setup(&env);
    env.as_contract(&client.address, || {
        let stored_oracle: Address = env.storage().persistent().get(&StorageKey::Oracle).unwrap();
        let stored_token: Address = env.storage().persistent().get(&StorageKey::Token).unwrap();
        let state: Outcome = env.storage().persistent().get(&StorageKey::State).unwrap();
        assert_eq!(stored_oracle, oracle);
        assert_eq!(stored_token, token);
        assert_eq!(state, Outcome::Undecided);
    });
}

#[test]
fn test_buy_updates_reserves() {
    let env = Env::default();
    let (client, _, _token, bettor1, bettor2, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    let (true_reserve_after_first, false_reserve_after_first) = client.get_reserves();
    assert_eq!(true_reserve_after_first, 1100); // 1000 + 100
    assert!(false_reserve_after_first < 1000); // Should decrease due to AMM
    
    client.buy(&bettor2, &200, &false);
    let (true_reserve_final, false_reserve_final) = client.get_reserves();
    assert_eq!(false_reserve_final, false_reserve_after_first + 200); // Increased by 200
    assert!(true_reserve_final < true_reserve_after_first); // Should decrease due to AMM
}

#[test]
#[should_panic(expected = "Market not live")]
fn test_buy_after_settle_panics() {
    let env = Env::default();
    let (client, oracle, _, bettor1, _, _) = setup(&env);
    client.settle(&oracle, &true);
    client.buy(&bettor1, &10, &true);
}

#[test]
#[should_panic(expected = "Amount must be positive")]
fn test_zero_buy_amount_panics() {
    let env = Env::default();
    let (client, _, _, bettor1, _, _) = setup(&env);
    client.buy(&bettor1, &0, &true);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_settle_panics() {
    let env = Env::default();
    let (client, _oracle, _, bettor1, _, _) = setup(&env);
    client.settle(&bettor1, &false);
}

#[test]
fn test_claim_before_settle_noop() {
    let env = Env::default();
    let (client, _oracle, token_id, bettor1, _, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    let mock_token = MockTokenClient::new(&env, &token_id);
    let before = mock_token.balance(&bettor1);
    client.claim(&bettor1);
    let after = mock_token.balance(&bettor1);
    assert_eq!(before, after);
}

#[test]
fn test_claim_winners_and_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    client.buy(&bettor2, &100, &false);
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
    let (client, oracle, _, bettor1, _, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    client.settle(&oracle, &true);
    client.claim(&bettor1);
    client.claim(&bettor1);
}

#[test]
fn multiple_bettors_correct_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2, _) = setup(&env);
    let bettor3 = Address::generate(&env);
    let bettor4 = Address::generate(&env);
    let initial = 1_000_i128;
    let mock_token = MockTokenClient::new(&env, &token_id);
    mock_token.mint(&bettor3, &initial);
    mock_token.approve(&bettor3, &client.address, &initial, &0_u32);
    mock_token.mint(&bettor4, &initial);
    mock_token.approve(&bettor4, &client.address, &initial, &0_u32);
    client.buy(&bettor1, &100, &true);
    client.buy(&bettor2, &200, &true);
    client.buy(&bettor3, &50, &false);
    client.buy(&bettor4, &100, &false);
    let user1_shares = client.get_user_shares(&bettor1).0;
    let user2_shares = client.get_user_shares(&bettor2).0;
    client.settle(&oracle, &true);
    let before1 = mock_token.balance(&bettor1);
    let before2 = mock_token.balance(&bettor2);
    let before3 = mock_token.balance(&bettor3);
    let before4 = mock_token.balance(&bettor4);
    client.claim(&bettor1);
    client.claim(&bettor2);
    client.claim(&bettor3);
    client.claim(&bettor4);
    let after1 = mock_token.balance(&bettor1);
    let after2 = mock_token.balance(&bettor2);
    let after3 = mock_token.balance(&bettor3);
    let after4 = mock_token.balance(&bettor4);
    assert_eq!(after1 - before1, user1_shares);
    assert_eq!(after2 - before2, user2_shares);
    assert_eq!(after3, before3); // Losers get nothing
    assert_eq!(after4, before4);
}

#[test]
fn test_initial_probabilities() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    let probabilities = client.get_current_probabilities();
    const SCALE: i128 = 1_000_000;
    assert_eq!(probabilities, (SCALE / 2, SCALE / 2)); // Should be 50/50 for balanced reserves
}

#[test]
fn test_amm_initial_market_reserves() {
    let env = Env::default();
    let (client, _, _, bettor1, _, _) = setup(&env);
    let market_info = client.get_market_info();
    assert_eq!(market_info, (1000, 1000, 0)); 
    client.buy(&bettor1, &100, &true);
    let market_info = client.get_market_info();
    assert_eq!(market_info.0, 1100); // true_reserve increased by 100
    assert!(market_info.1 < 1000); // false_reserve decreased due to AMM formula
    assert_eq!(market_info.2, 100); // total_volume should be 100 after the buy
}

#[test]
fn test_balanced_market_probabilities() {
    let env = Env::default();
    let (client, _, _, bettor1, bettor2, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    client.buy(&bettor2, &100, &false);
    let probabilities = client.get_current_probabilities();
    const SCALE: i128 = 1_000_000;
    assert!((probabilities.0 + probabilities.1 - SCALE).abs() <= 1);
    assert!(probabilities.0 >= SCALE / 4); // At least 25%
    assert!(probabilities.0 <= 3 * SCALE / 4); // At most 75%
    assert!(probabilities.1 >= SCALE / 4); // At least 25%  
    assert!(probabilities.1 <= 3 * SCALE / 4); // At most 75%
}

#[test]
fn test_imbalanced_market_probabilities() {
    let env = Env::default();
    let (client, _, _, bettor1, bettor2, _) = setup(&env);
    client.buy(&bettor1, &300, &true);
    client.buy(&bettor2, &100, &false);
    let probabilities = client.get_current_probabilities();
    assert!(probabilities.0 > probabilities.1);
}

#[test] 
fn test_amm_progressive_pricing() {
    let env = Env::default();
    let (client, _, _, bettor1, bettor2, _) = setup(&env);
    client.buy(&bettor1, &100, &true);
    let shares_first_100 = client.get_user_shares(&bettor1).0;
    client.buy(&bettor2, &100, &true);
    let shares_second_100 = client.get_user_shares(&bettor2).0;
    assert!(shares_first_100 > shares_second_100);
}

#[test]
fn test_constant_product_invariant() {
    let env = Env::default();
    let (client, _, _, bettor1, _, _) = setup(&env);
    let initial_k = client.get_constant_product();
    client.buy(&bettor1, &100, &true);
    let after_buy_k = client.get_constant_product();
    let diff = if initial_k > after_buy_k { initial_k - after_buy_k } else { after_buy_k - initial_k };
    assert!(diff <= 1000, "Constant product changed by more than tolerance: {} vs {}", initial_k, after_buy_k);
}

#[test]
fn test_initial_market_equal_pricing() {
    let env = Env::default();
    let (client, _, _, _, _, _) = setup(&env);
    let shares_true = client.get_buy_price(&100, &true);
    let shares_false = client.get_buy_price(&100, &false);
    assert_eq!(shares_true, shares_false);
}

#[test] 
fn test_minority_advantage_pricing() {
    let env = Env::default();
    let (client, _, _, bettor1, _, _) = setup(&env);
    client.buy(&bettor1, &400, &true);
    let shares_true = client.get_buy_price(&100, &true);
    let shares_false = client.get_buy_price(&100, &false);
    assert!(shares_false > shares_true);
}

#[test]
fn test_sell_functionality() {
    let env = Env::default();
    let (client, _, token_id, bettor1, _, _) = setup(&env);
    let mock_token = MockTokenClient::new(&env, &token_id);
    client.buy(&bettor1, &100, &true);
    let shares = client.get_user_shares(&bettor1).0;
    let initial_balance = mock_token.balance(&bettor1);
    let shares_to_sell = shares / 2;
    client.sell(&bettor1, &shares_to_sell, &true);
    let final_balance = mock_token.balance(&bettor1);
    let remaining_shares = client.get_user_shares(&bettor1).0;
    assert!(final_balance > initial_balance);
    assert_eq!(remaining_shares, shares - shares_to_sell);
}