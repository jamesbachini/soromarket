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
