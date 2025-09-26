#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Env, Address};
use crate::{PredictionMarketContract, PredictionMarketContractClient};

fn create_admin_and_client(env: &Env) -> (Address, PredictionMarketContractClient<'_>) {
    let admin = Address::generate(&env);
    let contract_id = env.register(PredictionMarketContract, ());
    let client = PredictionMarketContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    (admin, client)
}

#[test]
fn test_deposit_withdraw() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &1_000_000); // $1.00
    assert_eq!(client.get_balance(&user), 1_000_000);
    client.withdraw(&user, &400_000); // withdraw $0.40
    assert_eq!(client.get_balance(&user), 600_000);
}

#[test]
fn test_liquidity_provision() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.provide_liquidity(&admin, &5_000_000); // $5
    // Check LP balance through internal storage - no direct method exposed
    assert_eq!(client.total_liquidity(), 5_000_000);
    client.withdraw_liquidity(&admin, &2_000_000);
    assert_eq!(client.total_liquidity(), 3_000_000);
}

#[test]
fn test_create_market_and_bet() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    // Create market Brazil vs England with odds 40%, 25%, 34% => sums to 99
    client.create_market(&admin, &soroban_sdk::symbol_short!("BrazilEng"), &1234567890, &400_000, &250_000, &340_000);
    let market = client.get_market(&1); // Get the created market by ID
    assert_eq!(market.id, 1);
    assert_eq!(market.title, soroban_sdk::symbol_short!("BrazilEng"));
    client.place_bet(&user, &1, &0, &500_000); // market_id=1, outcome=0 (home)
    assert_eq!(client.get_balance(&user), 500_000);
}

#[test]
fn test_settlement() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    client.provide_liquidity(&admin, &20_000_000);
    client.deposit(&user1, &1_000_000);
    client.deposit(&user2, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("BrazilEng"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user1, &1, &0, &1_000_000); // bet on home (outcome 0)
    client.place_bet(&user2, &1, &1, &1_000_000); // bet on draw (outcome 1)
    client.settle_market(&admin, &1, &0); // settle market 1 with outcome 0 (home wins)
    // User1 should be paid (1m / 400k * 1m = 2.5m payout total, minus escrow logic)
    let bal1 = client.get_balance(&user1);
    let bal2 = client.get_balance(&user2);
    assert!(bal1 > 1_000_000); // user1 made profit
    assert_eq!(bal2, 0);       // user2 lost
}

// SECURITY & EDGE CASE TESTS

#[test]
#[should_panic(expected = "unauthorized: admin only")]
fn test_non_admin_create_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let attacker = Address::generate(&env);
    client.create_market(&attacker, &soroban_sdk::symbol_short!("Attack"), &1234567890, &400_000, &250_000, &340_000);
}

#[test]
#[should_panic(expected = "unauthorized: admin only")]
fn test_non_admin_settle_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let attacker = Address::generate(&env);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.settle_market(&attacker, &1, &0);
}

#[test]
#[should_panic(expected = "unauthorized: admin only")]
fn test_non_admin_update_odds() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let attacker = Address::generate(&env);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.update_odds(&attacker, &1, &300_000, &300_000, &390_000);
}

#[test]
#[should_panic(expected = "unauthorized: admin only")]
fn test_non_admin_archive_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let attacker = Address::generate(&env);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.archive_market(&attacker, &1);
}

#[test]
#[should_panic(expected = "odds below minimum")]
fn test_create_market_odds_too_low() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    // MIN_PRICE is 10,000 but we try with 5,000
    client.create_market(&admin, &soroban_sdk::symbol_short!("Invalid"), &1234567890, &5_000, &250_000, &735_000);
}

#[test]
#[should_panic(expected = "odds must sum to $0.99")]
fn test_create_market_odds_wrong_sum() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    // Sum = 1_000_000 instead of 990_000
    client.create_market(&admin, &soroban_sdk::symbol_short!("Invalid"), &1234567890, &400_000, &300_000, &300_000);
}

#[test]
#[should_panic(expected = "deposit positive")]
fn test_deposit_zero_amount() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &0);
}

#[test]
#[should_panic(expected = "deposit positive")]
fn test_deposit_negative_amount() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &-100_000);
}

#[test]
#[should_panic(expected = "withdraw positive")]
fn test_withdraw_zero_amount() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &1_000_000);
    client.withdraw(&user, &0);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_withdraw_more_than_balance() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &500_000);
    client.withdraw(&user, &600_000);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_bet_more_than_balance() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &500_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &0, &600_000);
}

#[test]
#[should_panic(expected = "bet amount positive")]
fn test_bet_zero_amount() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &0, &0);
}

#[test]
#[should_panic(expected = "invalid outcome")]
fn test_bet_invalid_outcome() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &3, &500_000);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_bet_nonexistent_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &1_000_000);
    client.place_bet(&user, &999, &0, &500_000);
}

#[test]
#[should_panic(expected = "market not active")]
fn test_bet_on_settled_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &2_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &0, &1_000_000);
    client.settle_market(&admin, &1, &0);
    client.place_bet(&user, &1, &0, &500_000); // Should fail
}

#[test]
#[should_panic(expected = "market not active")]
fn test_bet_on_archived_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.archive_market(&admin, &1);
    client.place_bet(&user, &1, &0, &500_000);
}

#[test]
#[should_panic(expected = "invalid outcome")]
fn test_settle_invalid_outcome() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.settle_market(&admin, &1, &3);
}

#[test]
#[should_panic(expected = "market not active")]
fn test_double_settlement() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &0, &500_000);
    client.settle_market(&admin, &1, &0);
    client.settle_market(&admin, &1, &0); // Should fail
}

#[test]
#[should_panic(expected = "insufficient liquidity for payouts")]
fn test_insufficient_liquidity_for_settlement() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    // Only provide minimal liquidity
    client.provide_liquidity(&admin, &1_000_000);
    client.deposit(&user, &10_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    // Bet large amount that would require more payout than available liquidity
    client.place_bet(&user, &1, &0, &10_000_000);
    // Settlement should fail due to insufficient liquidity
    client.settle_market(&admin, &1, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_provide_liquidity_zero() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let provider = Address::generate(&env);
    client.provide_liquidity(&provider, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_withdraw_liquidity_zero() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let provider = Address::generate(&env);
    client.provide_liquidity(&provider, &1_000_000);
    client.withdraw_liquidity(&provider, &0);
}

#[test]
#[should_panic(expected = "insufficient lp balance")]
fn test_withdraw_more_liquidity_than_provided() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let provider = Address::generate(&env);
    client.provide_liquidity(&provider, &1_000_000);
    client.withdraw_liquidity(&provider, &2_000_000);
}

#[test]
fn test_market_bettor_count_tracking() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user1, &1_000_000);
    client.deposit(&user2, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    assert_eq!(client.get_bettor_count(&1), 0);
    client.place_bet(&user1, &1, &0, &500_000);
    assert_eq!(client.get_bettor_count(&1), 1);
    client.place_bet(&user2, &1, &1, &500_000);
    assert_eq!(client.get_bettor_count(&1), 2);
}

#[test]
fn test_market_bettor_count_increments() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    // Test that bettor count increments correctly
    for i in 0..10u32 {
        let user = Address::generate(&env);
        client.deposit(&user, &100_000);
        client.place_bet(&user, &1, &(i % 3), &50_000);
        assert_eq!(client.get_bettor_count(&1), i + 1);
    }
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register(PredictionMarketContract, ());
    let client = PredictionMarketContractClient::new(&env, &contract_id);
    client.initialize(&admin);
    client.initialize(&admin); // Should fail
}

#[test]
fn test_balance_consistency_after_multiple_operations() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user1, &1_000_000);
    client.deposit(&user2, &2_000_000);

    let initial_total_liq = client.total_liquidity();
    assert_eq!(initial_total_liq, 13_000_000); // 10M + 1M + 2M

    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user1, &1, &0, &500_000);
    client.place_bet(&user2, &1, &1, &1_000_000);

    // Liquidity should include the bet amounts
    assert_eq!(client.total_liquidity(), 14_500_000); // 13M + 0.5M + 1M
    assert_eq!(client.get_balance(&user1), 500_000);
    assert_eq!(client.get_balance(&user2), 1_000_000);
}

#[test]
fn test_normal_settlement_works() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);

    // Create market with valid odds
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_bet(&user, &1, &0, &500_000);

    // Normal settlement should work fine with valid prices
    client.settle_market(&admin, &1, &0);

    // Verify user received payout
    assert!(client.get_balance(&user) > 500_000);
}

#[test]
#[should_panic(expected = "overflow")]
fn test_arithmetic_overflow_protection() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);

    // Test with maximum possible amounts to trigger overflow
    let max_amount = i128::MAX;

    // First deposit should work
    client.deposit(&user, &1);

    // Attempting to deposit MAX should cause overflow protection
    client.deposit(&user, &max_amount); // Should panic due to overflow in credit_user_balance
}

#[test]
fn test_payout_calculation_precision() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);

    client.provide_liquidity(&admin, &100_000_000);
    client.deposit(&user, &1_000_000);

    // Test with precise odds calculations
    // Odds: home=40%, draw=25%, away=34% (sum = 99%)
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    let bet_amount = 1_000_000i128; // $1.00
    client.place_bet(&user, &1, &0, &bet_amount);

    // Calculate expected payout: $1 * 1,000,000 / 400,000 = $2.50
    let expected_payout = bet_amount * 1_000_000i128 / 400_000i128;
    assert_eq!(expected_payout, 2_500_000); // $2.50

    client.settle_market(&admin, &1, &0);
    assert_eq!(client.get_balance(&user), expected_payout);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_get_nonexistent_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    client.get_market(&999);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_get_bettor_count_nonexistent_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    client.get_bettor_count(&999);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_update_odds_nonexistent_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.update_odds(&admin, &999, &400_000, &250_000, &340_000);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_settle_nonexistent_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.settle_market(&admin, &999, &0);
}
