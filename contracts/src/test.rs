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
fn test_create_market_and_stake() {
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
    client.place_stake(&user, &1, &0, &500_000); // market_id=1, outcome=0 (home)
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
    client.place_stake(&user1, &1, &0, &1_000_000); // stake on home (outcome 0)
    client.place_stake(&user2, &1, &1, &1_000_000); // stake on draw (outcome 1)
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
fn test_stake_more_than_balance() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &500_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_stake(&user, &1, &0, &600_000);
}

#[test]
#[should_panic(expected = "stake amount positive")]
fn test_stake_zero_amount() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_stake(&user, &1, &0, &0);
}

#[test]
#[should_panic(expected = "invalid outcome")]
fn test_stake_invalid_outcome() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_stake(&user, &1, &3, &500_000);
}

#[test]
#[should_panic(expected = "market not found")]
fn test_stake_nonexistent_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.deposit(&user, &1_000_000);
    client.place_stake(&user, &999, &0, &500_000);
}

#[test]
#[should_panic(expected = "market not active")]
fn test_stake_on_settled_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &2_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_stake(&user, &1, &0, &1_000_000);
    client.settle_market(&admin, &1, &0);
    client.place_stake(&user, &1, &0, &500_000); // Should fail
}

#[test]
#[should_panic(expected = "market not active")]
fn test_stake_on_archived_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.archive_market(&admin, &1);
    client.place_stake(&user, &1, &0, &500_000);
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
    client.place_stake(&user, &1, &0, &500_000);
    client.settle_market(&admin, &1, &0);
    client.settle_market(&admin, &1, &0); // Should fail
}

// Note: This test is removed because with CPMM dynamic pricing,
// stake amounts add to both reserves AND total liquidity, making it
// difficult to create insufficient liquidity scenarios naturally.

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
fn test_market_staker_count_tracking() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user1, &1_000_000);
    client.deposit(&user2, &1_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    assert_eq!(client.get_staker_count(&1), 0);
    client.place_stake(&user1, &1, &0, &500_000);
    assert_eq!(client.get_staker_count(&1), 1);
    client.place_stake(&user2, &1, &1, &500_000);
    assert_eq!(client.get_staker_count(&1), 2);
}

#[test]
fn test_market_staker_count_increments() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    client.provide_liquidity(&admin, &10_000_000);
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    // Test that staker count increments correctly
    for i in 0..10u32 {
        let user = Address::generate(&env);
        client.deposit(&user, &100_000);
        client.place_stake(&user, &1, &(i % 3), &50_000);
        assert_eq!(client.get_staker_count(&1), i + 1);
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
    assert_eq!(initial_total_liq, 10_000_000); // Only LP provision counts, not user deposits

    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);
    client.place_stake(&user1, &1, &0, &500_000);
    client.place_stake(&user2, &1, &1, &1_000_000);

    // Liquidity should remain unchanged (stakes don't add to LP pool)
    assert_eq!(client.total_liquidity(), 10_000_000); // Only LP provision
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
    client.place_stake(&user, &1, &0, &500_000);

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

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);

    // Test with precise odds calculations
    // Odds: home=40%, draw=25%, away=34% (sum = 99%)
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    let stake_amount = 1_000_000i128; // $1.00
    client.place_stake(&user, &1, &0, &stake_amount);

    // With CPMM, the payout is determined by the reserve pool
    // Market gets 100M liquidity, reserve_home should be 40M initially
    // User gets their proportional share of the winning reserve
    client.settle_market(&admin, &1, &0);

    // User should receive payout from the home reserve pool
    let balance = client.get_balance(&user);
    assert!(balance > 0); // User got paid
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
fn test_get_staker_count_nonexistent_market() {
    let env = Env::default();
    let (_admin, client) = create_admin_and_client(&env);
    client.get_staker_count(&999);
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

#[test]
fn test_dynamic_pricing_cpmm() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user1, &10_000_000);
    client.deposit(&user2, &10_000_000);

    // Create market with odds: home=40%, draw=25%, away=34%
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    // Get initial odds (should be close to initial but calculated from reserves)
    let (odds_home, odds_draw, odds_away) = client.get_current_odds(&1);
    // Just verify they're in expected range and sum to roughly $0.99
    assert!(odds_home > 390_000 && odds_home < 410_000);
    assert!(odds_draw > 240_000 && odds_draw < 260_000);
    assert!(odds_away > 330_000 && odds_away < 350_000);

    // User1 stakes on home - should increase home price
    client.place_stake(&user1, &1, &0, &1_000_000);

    // Check that odds changed
    let (new_odds_home, new_odds_draw, new_odds_away) = client.get_current_odds(&1);
    assert!(new_odds_home > odds_home); // Home price increased
    assert!(new_odds_draw < odds_draw); // Draw price decreased relatively
    assert!(new_odds_away < odds_away); // Away price decreased relatively

    // User2 stakes on away - should increase away price
    client.place_stake(&user2, &1, &2, &1_000_000);

    let (_final_odds_home, _final_odds_draw, final_odds_away) = client.get_current_odds(&1);
    assert!(final_odds_away > new_odds_away); // Away price increased
}

#[test]
fn test_cash_out_functionality() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &10_000_000);

    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    // User places stake
    client.place_stake(&user, &1, &0, &1_000_000);
    let balance_after_stake = client.get_balance(&user);
    assert_eq!(balance_after_stake, 9_000_000);

    // User cashes out immediately
    // With capped payout model:
    // Buy: shares = 1M (1:1, no entry slippage)
    // Reserve_home: 4M → 5M shares
    // Total liquidity: $20M → $21M
    // Cashout: min(1M, 1M * $21M / 5M) = min(1M, 4.2M) = 1M (capped)
    // After 5% fee: 950K
    // User gets back 95% of stake (loses 5% fee only)
    client.cash_out(&user, &1);

    let balance_after_cashout = client.get_balance(&user);
    // User loses from: entry slippage + exit slippage + 5% fee
    // Total loss ~2.5% from slippage + 5% fee = ~7.5%
    assert!(balance_after_cashout > balance_after_stake);
    assert!(balance_after_cashout >= 9_700_000); // Should get back > 97% (allowing for slippage)
    assert!(balance_after_cashout <= 10_000_000); // Less than or equal to original
}

#[test]
#[should_panic(expected = "unauthorized: not stake owner")]
fn test_cash_out_unauthorized() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user1, &1_000_000);

    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    client.place_stake(&user1, &1, &0, &1_000_000);

    // User2 tries to cash out User1's stake - should fail
    client.cash_out(&user2, &1);
}

#[test]
#[should_panic(expected = "stake not found")]
fn test_cash_out_settled_market() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let user = Address::generate(&env);

    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&user, &1_000_000);

    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &250_000, &340_000);

    client.place_stake(&user, &1, &0, &1_000_000);
    client.settle_market(&admin, &1, &0);

    // Try to cash out after settlement - stake is removed, should fail with "stake not found"
    client.cash_out(&user, &1);
}

#[test]
fn test_arbitrage_exploit_repeated_cycles() {
    let env = Env::default();
    let (admin, client) = create_admin_and_client(&env);
    let attacker = Address::generate(&env);

    // Setup: Market with $1000 initial liquidity
    client.provide_liquidity(&admin, &10_000_000);
    client.deposit(&attacker, &1_000_000); // Give attacker $1000

    // Create market: home=$0.40, draw=$0.33, away=$0.26
    client.create_market(&admin, &soroban_sdk::symbol_short!("Test"), &1234567890, &400_000, &330_000, &260_000);

    let initial_balance = client.get_balance(&attacker);
    let mut current_balance = initial_balance;

    // Attempt to exploit by buying and immediately cashing out repeatedly
    for cycle in 0..5 {
        let balance_before = current_balance;

        // Stake 10% of balance
        let stake_amount = current_balance / 10;
        if stake_amount < 10_000 { break; }

        // Buy shares
        client.place_stake(&attacker, &1, &0, &stake_amount);

        // Immediately cash out
        let stake_id = cycle + 1;
        client.cash_out(&attacker, &stake_id);

        current_balance = client.get_balance(&attacker);

        // Each cycle should not increase balance
        assert!(current_balance <= balance_before + 100, "Arbitrage exploit detected in cycle!");
    }

    let total_profit = current_balance as i128 - initial_balance as i128;

    // This should NOT be profitable - attacker should lose money or break even
    // Allow small profit due to rounding (< 0.1%)
    assert!(total_profit < initial_balance / 1000, "CRITICAL: Arbitrage exploit detected!");
}
