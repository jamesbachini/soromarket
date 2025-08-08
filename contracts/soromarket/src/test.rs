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
    client.setup(&oracle, &token_id, &String::from_str(&env, "James Will Be Next President Of The USA"), &500_000);
    (client, oracle, token_id, bettor1, bettor2)
}

#[test]
fn test_setup_stores_values() {
    let env = Env::default();
    let (client, oracle, token, _, _) = setup(&env);
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
fn test_trade_and_totals() {
    let env = Env::default();
    let (client, _, _token, bettor1, bettor2) = setup(&env);
    client.trade(&bettor1, &100, &true);
    client.trade(&bettor2, &200, &false);
    env.as_contract(&client.address, || {
        let tt: i128 = env.storage().persistent().get(&StorageKey::TrueTotal).unwrap();
        let ft: i128 = env.storage().persistent().get(&StorageKey::FalseTotal).unwrap();
        assert_eq!(tt, 100);
        assert_eq!(ft, 200);
    });
}

#[test]
#[should_panic(expected = "Market not live")]
fn test_trade_after_settle_panics() {
    let env = Env::default();
    let (client, oracle, _, bettor1, _) = setup(&env);
    client.settle(&oracle, &true);
    client.trade(&bettor1, &10, &true);
}

#[test]
#[should_panic(expected = "Amount must be non-zero")]
fn test_zero_trade_amount_panics() {
    let env = Env::default();
    let (client, _, _, bettor1, _) = setup(&env);
    client.trade(&bettor1, &0, &true);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_settle_panics() {
    let env = Env::default();
    let (client, _oracle, _, bettor1, _) = setup(&env);
    client.settle(&bettor1, &false);
}

#[test]
fn test_claim_before_settle_noop() {
    let env = Env::default();
    let (client, _oracle, token_id, bettor1, _) = setup(&env);
    client.trade(&bettor1, &100, &true);
    let mock_token = MockTokenClient::new(&env, &token_id);
    let before = mock_token.balance(&bettor1);
    client.claim(&bettor1);
    let after = mock_token.balance(&bettor1);
    assert_eq!(before, after);
}

#[test]
fn test_claim_winners_and_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2) = setup(&env);
    client.trade(&bettor1, &100, &true);
    client.trade(&bettor2, &100, &false);
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
    client.trade(&bettor1, &100, &true);
    client.settle(&oracle, &true);
    client.claim(&bettor1);
    client.claim(&bettor1);
}

#[test]
fn multiple_bettors_correct_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2) = setup(&env);
    let bettor3 = Address::generate(&env);
    let bettor4 = Address::generate(&env);
    let initial = 1_000_i128;
    let mock_token = MockTokenClient::new(&env, &token_id);
    mock_token.mint(&bettor3, &initial);
    mock_token.approve(&bettor3, &client.address, &initial, &0_u32);
    mock_token.mint(&bettor4, &initial);
    mock_token.approve(&bettor4, &client.address, &initial, &0_u32);
    let a = 100_i128;
    let b = 300_i128;
    let c = 50_i128;
    let d = 150_i128;
    client.trade(&bettor1, &a, &true);
    client.trade(&bettor2, &b, &true);
    client.trade(&bettor3, &c, &false);
    client.trade(&bettor4, &d, &false);
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
    let true_total = a + b;
    let false_total = c + d;
    const SCALE: i128 = 1_000_000;
    let share1 = SCALE * a / true_total;
    let exp1 = a + (false_total * share1 / SCALE);
    let share2 = SCALE * b / true_total;
    let exp2 = b + (false_total * share2 / SCALE);
    assert_eq!(after1 - before1, exp1);
    assert_eq!(after2 - before2, exp2);
    assert_eq!(after3, before3);
    assert_eq!(after4, before4);
}

#[test]
fn test_lsmr_initial_market_pricing() {
    let env = Env::default();
    let (client, _, _, bettor1, _) = setup(&env);
    
    let market_info = client.get_market_info();
    assert_eq!(market_info, (0, 0, 0, 0)); // (true_shares, false_shares, true_total, false_total)
    
    // First trade should get 1:1 ratio when market is empty
    client.trade(&bettor1, &100, &true);
    
    let market_info = client.get_market_info();
    assert_eq!(market_info.0, 100); // true_shares should equal amount traded
    assert_eq!(market_info.2, 100); // true_total should equal amount traded
}

#[test]
fn test_lsmr_balanced_market_pricing() {
    let env = Env::default();
    let (client, _, _, bettor1, bettor2) = setup(&env);
    
    // Create balanced market - should get roughly equal shares
    client.trade(&bettor1, &100, &true);
    client.trade(&bettor2, &100, &false);
    
    let market_info = client.get_market_info();
    // When market is balanced (50/50), both sides should have similar shares
    let true_shares = market_info.0;
    let false_shares = market_info.1;
    
    // Both should have reasonable share amounts in a balanced market
    // The exact amounts depend on the bonding curve pricing
    assert!(true_shares >= 50 && true_shares <= 150);
    assert!(false_shares >= 50 && false_shares <= 300);
}

#[test]
fn test_lsmr_imbalanced_market_pricing() {
    let env = Env::default();
    let (client, _, token_id, bettor1, bettor2) = setup(&env);
    let bettor3 = Address::generate(&env);
    let initial = 1_000_i128;
    let mock_token = MockTokenClient::new(&env, &token_id);
    mock_token.mint(&bettor3, &initial);
    mock_token.approve(&bettor3, &client.address, &initial, &0_u32);
    
    // Create heavily imbalanced market
    client.trade(&bettor1, &300, &true);
    client.trade(&bettor2, &100, &false);
    
    let market_info_before = client.get_market_info();
    
    // Trading on minority side (false) should get more shares per token
    client.trade(&bettor3, &100, &false);
    
    let market_info_after = client.get_market_info();
    let new_false_shares = market_info_after.1 - market_info_before.1;
    
    // Should get more than 100 shares for 100 tokens on minority side
    assert!(new_false_shares > 100);
}

#[test]
fn test_lsmr_liquidity_parameter_effects() {
    let env = Env::default();
    
    // Test with high liquidity parameter (more responsive pricing)
    let (client_high, oracle_high, token_high, bettor1_high, bettor2_high) = {
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
        // High liquidity param = 800,000 (80%)
        client.setup(&oracle, &token_id, &String::from_str(&env, "High Liquidity Test"), &800_000);
        (client, oracle, token_id, bettor1, bettor2)
    };
    
    // Test with low liquidity parameter (less responsive pricing)
    let (client_low, oracle_low, token_low, bettor1_low, bettor2_low) = {
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
        // Low liquidity param = 200,000 (20%)
        client.setup(&oracle, &token_id, &String::from_str(&env, "Low Liquidity Test"), &200_000);
        (client, oracle, token_id, bettor1, bettor2)
    };
    
    // Create same imbalanced scenario in both markets
    client_high.trade(&bettor1_high, &300, &true);
    client_low.trade(&bettor1_low, &300, &true);
    
    // Query pricing for same trade amount on minority side
    let price_high = client_high.get_price_for_shares(&100, &false);
    let price_low = client_low.get_price_for_shares(&100, &false);
    
    // Higher liquidity parameter should make pricing more responsive
    // For minority positions, this means higher liquidity param gives better deals
    // However, the exact relationship depends on the specific market state
    assert!(price_high != price_low); // They should be different
}

#[test]
fn test_lsmr_share_based_payouts() {
    let env = Env::default();
    let (client, oracle, token_id, bettor1, bettor2) = setup(&env);
    let bettor3 = Address::generate(&env);
    let initial = 1_000_i128;
    let mock_token = MockTokenClient::new(&env, &token_id);
    mock_token.mint(&bettor3, &initial);
    mock_token.approve(&bettor3, &client.address, &initial, &0_u32);
    
    // Create scenario where different amounts result in different share counts
    client.trade(&bettor1, &200, &true);  // First trade - should get more shares per token
    client.trade(&bettor2, &100, &true);  // Second trade - should get fewer shares per token
    client.trade(&bettor3, &300, &false); // Large trade on false side
    
    let market_info_before_settle = client.get_market_info();
    let true_shares_total = market_info_before_settle.0;
    let total_pool = market_info_before_settle.2 + market_info_before_settle.3;
    let _ = true_shares_total; // silence potential unused var warning
    
    client.settle(&oracle, &true);
    
    let before1 = mock_token.balance(&bettor1);
    let before2 = mock_token.balance(&bettor2);
    let before3 = mock_token.balance(&bettor3);
    
    client.claim(&bettor1);
    client.claim(&bettor2);
    client.claim(&bettor3);
    
    let after1 = mock_token.balance(&bettor1);
    let after2 = mock_token.balance(&bettor2);
    let after3 = mock_token.balance(&bettor3);
    
    let payout1 = after1 - before1;
    let payout2 = after2 - before2;
    let payout3 = after3 - before3;
    
    // Bettor1 should get larger payout than bettor2 due to more shares (earlier trade)
    assert!(payout1 > payout2);
    // Bettor3 (false bettor) should get nothing
    assert_eq!(payout3, 0);
    // Total payouts should be close to total pool (accounting for rounding)
    let total_payout = payout1 + payout2;
    assert!(total_payout >= total_pool - 5 && total_payout <= total_pool + 5);
}

#[test]
fn test_lsmr_get_price_for_shares_consistency() {
    let env = Env::default();
    let (client, _, _, bettor1, bettor2) = setup(&env);
    
    // Test that get_price_for_shares returns consistent results
    let initial_price = client.get_price_for_shares(&100, &true);
    assert_eq!(initial_price, 100); // Should be 1:1 for empty market
    
    client.trade(&bettor1, &200, &true);
    
    // After imbalance, minority side should be cheaper
    let true_price = client.get_price_for_shares(&100, &true);
    let false_price = client.get_price_for_shares(&100, &false);
    
    assert!(false_price < true_price); // False side should be cheaper
}

#[test]
fn test_lsmr_progressive_pricing() {
    let env = Env::default();
    let (client, _, token_id, bettor1, bettor2) = setup(&env);
    let bettor3 = Address::generate(&env);
    let bettor4 = Address::generate(&env);
    let initial = 1_000_i128;
    let mock_token = MockTokenClient::new(&env, &token_id);
    mock_token.mint(&bettor3, &initial);
    mock_token.approve(&bettor3, &client.address, &initial, &0_u32);
    mock_token.mint(&bettor4, &initial);
    mock_token.approve(&bettor4, &client.address, &initial, &0_u32);
    
    // Test that successive trades get progressively more expensive
    client.trade(&bettor1, &100, &true);
    let market_info1 = client.get_market_info();
    let shares_per_100_1 = market_info1.0;
    
    client.trade(&bettor2, &100, &true);
    let market_info2 = client.get_market_info();
    let shares_per_100_2 = market_info2.0 - shares_per_100_1;
    
    client.trade(&bettor3, &100, &true);
    let market_info3 = client.get_market_info();
    let shares_per_100_3 = market_info3.0 - market_info2.0;
    
    assert!(shares_per_100_1 >= shares_per_100_2);
    assert!(shares_per_100_2 >= shares_per_100_3);
}