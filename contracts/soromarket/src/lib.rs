#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Env, Address, String};
use sep_41_token::TokenClient;

#[contract]
pub struct SoroMarket;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum Outcome {
    Undecided,
    TrueOutcome,
    FalseOutcome,
}

#[contracttype]
pub enum StorageKey {
    Oracle,
    Token,
    Market,
    State,
    TrueReserve,
    FalseReserve,
    TotalVolume,
    TrueDeposits,
    FalseDeposits,
    UserTrueShares(Address),
    UserFalseShares(Address),
    Claimed(Address),
}

#[contractimpl]
impl SoroMarket {
    pub fn setup(env: Env, deployer: Address, oracle: Address, token: Address, market: String, initial_reserve: i128) {
        deployer.require_auth();
        let store = env.storage().persistent();
        if store.get::<_, Address>(&StorageKey::Oracle).is_some() {
            panic!("Market already setup");
        }
        let total_liquidity = initial_reserve.checked_mul(2).expect("liquidity overflow");
        TokenClient::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &deployer,
            &env.current_contract_address(),
            &total_liquidity,
        );
        store.set(&StorageKey::Oracle, &oracle);
        store.set(&StorageKey::Token, &token);
        store.set(&StorageKey::TrueReserve, &initial_reserve);
        store.set(&StorageKey::FalseReserve, &initial_reserve);
        store.set(&StorageKey::TrueDeposits, &initial_reserve);
        store.set(&StorageKey::FalseDeposits, &initial_reserve);
        store.set(&StorageKey::TotalVolume, &total_liquidity);
        store.set(&StorageKey::Market, &market);
        store.set(&StorageKey::State, &Outcome::Undecided);
    }

    pub fn buy(env: Env, user: Address, amount: i128, bet_on_true: bool) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        assert_eq!(state, Outcome::Undecided, "Market not live");
        assert!(amount > 0, "Amount must be positive"); 
        let token: Address = store.get(&StorageKey::Token).unwrap();
        let mut true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let mut false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let mut true_deposits: i128 = store.get(&StorageKey::TrueDeposits).unwrap();
        let mut false_deposits: i128 = store.get(&StorageKey::FalseDeposits).unwrap();
        let k = true_reserve.checked_mul(false_reserve).expect("k overflow");
        let shares_received = if bet_on_true {
            let new_true_reserve = true_reserve.checked_add(amount).expect("reserve overflow");
            let new_false_reserve = k / new_true_reserve;
            assert!(new_false_reserve > 0, "Reserve would become zero");
            let shares = false_reserve - new_false_reserve;
            true_reserve = new_true_reserve;
            false_reserve = new_false_reserve;
            true_deposits = true_deposits.checked_add(amount).expect("deposit overflow");
            shares
        } else {
            let new_false_reserve = false_reserve.checked_add(amount).expect("reserve overflow");
            let new_true_reserve = k / new_false_reserve;
            assert!(new_true_reserve > 0, "Reserve would become zero");
            let shares = true_reserve - new_true_reserve;
            false_reserve = new_false_reserve;
            true_reserve = new_true_reserve;
            false_deposits = false_deposits.checked_add(amount).expect("deposit overflow");
            shares
        };
        assert!(shares_received > 0, "Zero shares received");
        TokenClient::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &user,
            &env.current_contract_address(),
            &amount,
        );
        let user_key = if bet_on_true {
            StorageKey::UserTrueShares(user.clone())
        } else {
            StorageKey::UserFalseShares(user.clone())
        };
        let current_shares = store.get(&user_key).unwrap_or(0_i128);
        let new_user_shares = current_shares.checked_add(shares_received).expect("user shares overflow");
        let current_volume: i128 = store.get(&StorageKey::TotalVolume).unwrap_or(0);
        let new_volume = current_volume.checked_add(amount).expect("volume overflow");
        store.set(&StorageKey::TrueReserve, &true_reserve);
        store.set(&StorageKey::FalseReserve, &false_reserve);
        store.set(&StorageKey::TrueDeposits, &true_deposits);
        store.set(&StorageKey::FalseDeposits, &false_deposits);
        store.set(&StorageKey::TotalVolume, &new_volume);
        store.set(&user_key, &new_user_shares);
    }
    
    pub fn sell(env: Env, user: Address, shares: i128, bet_on_true: bool) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        assert_eq!(state, Outcome::Undecided, "Market not live");
        assert!(shares > 0, "Shares must be positive");
        let user_key = if bet_on_true {
            StorageKey::UserTrueShares(user.clone())
        } else {
            StorageKey::UserFalseShares(user.clone())
        };
        let current_shares = store.get(&user_key).unwrap_or(0_i128);
        assert!(current_shares >= shares, "Not enough shares to sell");
        let mut true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let mut false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let mut true_deposits: i128 = store.get(&StorageKey::TrueDeposits).unwrap();
        let mut false_deposits: i128 = store.get(&StorageKey::FalseDeposits).unwrap();
        let k = true_reserve.checked_mul(false_reserve).expect("k overflow");
        let payout = if bet_on_true {
            let new_false_reserve = false_reserve.checked_add(shares).expect("reserve overflow");
            let new_true_reserve = k / new_false_reserve;
            let payout = true_reserve - new_true_reserve;
            true_reserve = new_true_reserve;
            false_reserve = new_false_reserve;
            true_deposits = true_deposits.checked_sub(payout).expect("deposit underflow");
            payout
        } else {
            let new_true_reserve = true_reserve.checked_add(shares).expect("reserve overflow");
            let new_false_reserve = k / new_true_reserve;
            let payout = false_reserve - new_false_reserve;
            false_reserve = new_false_reserve;
            true_reserve = new_true_reserve;
            false_deposits = false_deposits.checked_sub(payout).expect("deposit underflow");
            payout
        };
        assert!(payout > 0, "Zero payout");
        let new_user_shares = current_shares - shares;
        let current_volume: i128 = store.get(&StorageKey::TotalVolume).unwrap_or(0);
        let new_volume = current_volume.checked_add(payout).expect("volume overflow");
        store.set(&StorageKey::TrueReserve, &true_reserve);
        store.set(&StorageKey::FalseReserve, &false_reserve);
        store.set(&StorageKey::TrueDeposits, &true_deposits);
        store.set(&StorageKey::FalseDeposits, &false_deposits);
        store.set(&StorageKey::TotalVolume, &new_volume);
        store.set(&user_key, &new_user_shares);
        let token: Address = store.get(&StorageKey::Token).unwrap();
        TokenClient::new(&env, &token).transfer(
            &env.current_contract_address(),
            &user,
            &payout,
        );
    }

    pub fn settle(env: Env, oracle: Address, outcome: bool) {
        oracle.require_auth();
        let store = env.storage().persistent();
        let stored: Address = store.get(&StorageKey::Oracle).unwrap();
        assert_eq!(oracle, stored, "Unauthorized");
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        assert_eq!(state, Outcome::Undecided, "Already settled");
        let new_state = if outcome {
            Outcome::TrueOutcome
        } else {
            Outcome::FalseOutcome
        };
        store.set(&StorageKey::State, &new_state);
    }

    pub fn claim(env: Env, user: Address) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        if state == Outcome::Undecided {
            return;
        }
        let already_claimed: bool = store
            .get(&StorageKey::Claimed(user.clone()))
            .unwrap_or(false);
        assert!(!already_claimed, "Already claimed");
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let _total_pool = true_reserve + false_reserve;
        let user_true: i128 = store
            .get(&StorageKey::UserTrueShares(user.clone()))
            .unwrap_or(0);
        let user_false: i128 = store
            .get(&StorageKey::UserFalseShares(user.clone()))
            .unwrap_or(0); 
        let winnings = if state == Outcome::TrueOutcome && user_true > 0 {
            user_true
        } else if state == Outcome::FalseOutcome && user_false > 0 {
            user_false
        } else {
            0
        };
        store.set(&StorageKey::UserTrueShares(user.clone()), &0i128);
        store.set(&StorageKey::UserFalseShares(user.clone()), &0i128);
        if winnings > 0 {
            let token: Address = store.get(&StorageKey::Token).unwrap();
            TokenClient::new(&env, &token).transfer(
                &env.current_contract_address(),
                &user,
                &winnings,
            );
        }
        store.set(&StorageKey::Claimed(user), &true);
    }

    pub fn get_buy_price(env: Env, amount: i128, bet_on_true: bool) -> i128 {
        let store = env.storage().persistent();
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let k = true_reserve.checked_mul(false_reserve).expect("k overflow");
        if bet_on_true {
            let new_true_reserve = true_reserve.checked_add(amount).expect("reserve overflow");
            let new_false_reserve = k / new_true_reserve;
            false_reserve - new_false_reserve
        } else {
            let new_false_reserve = false_reserve.checked_add(amount).expect("reserve overflow");
            let new_true_reserve = k / new_false_reserve;
            true_reserve - new_true_reserve
        }
    }
    
    pub fn get_sell_price(env: Env, shares: i128, bet_on_true: bool) -> i128 {
        let store = env.storage().persistent();
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let k = true_reserve.checked_mul(false_reserve).expect("k overflow");
        if bet_on_true {
            let new_false_reserve = false_reserve.checked_add(shares).expect("reserve overflow");
            let new_true_reserve = k / new_false_reserve;
            true_reserve - new_true_reserve
        } else {
            let new_true_reserve = true_reserve.checked_add(shares).expect("reserve overflow");
            let new_false_reserve = k / new_true_reserve;
            false_reserve - new_false_reserve
        }
    }

    pub fn get_market_info(env: Env) -> (i128, i128, i128) {
        let store = env.storage().persistent();
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let total_volume: i128 = store.get(&StorageKey::TotalVolume).unwrap_or(0);
        (true_reserve, false_reserve, total_volume)
    }

    pub fn get_market_description(env: Env) -> String {
        let store = env.storage().persistent();
        store.get(&StorageKey::Market).unwrap()
    }

    pub fn get_market_state(env: Env) -> Outcome {
        let store = env.storage().persistent();
        store.get(&StorageKey::State).unwrap()
    }

    pub fn get_user_shares(env: Env, user: Address) -> (i128, i128) {
        let store = env.storage().persistent();
        let t = store.get(&StorageKey::UserTrueShares(user.clone())).unwrap_or(0);
        let f = store.get(&StorageKey::UserFalseShares(user)).unwrap_or(0);
        (t, f)
    }

    pub fn get_current_probabilities(env: Env) -> (i128, i128) {
        const SCALE: i128 = 1_000_000;
        let store = env.storage().persistent();
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        let total_reserve = true_reserve + false_reserve;
        if total_reserve == 0 {
            return (SCALE / 2, SCALE / 2);
        }
        let true_prob = SCALE * true_reserve / total_reserve;
        let false_prob = SCALE * false_reserve / total_reserve;
        (true_prob, false_prob)
    }


    pub fn get_oracle(env: Env) -> Address {
        let store = env.storage().persistent();
        store.get(&StorageKey::Oracle).unwrap()
    }

    pub fn get_token(env: Env) -> Address {
        let store = env.storage().persistent();
        store.get(&StorageKey::Token).unwrap()
    }

    pub fn get_reserves(env: Env) -> (i128, i128) {
        let store = env.storage().persistent();
        let true_reserve: i128 = store.get(&StorageKey::TrueReserve).unwrap();
        let false_reserve: i128 = store.get(&StorageKey::FalseReserve).unwrap();
        (true_reserve, false_reserve)
    }
    
    pub fn get_constant_product(env: Env) -> i128 {
        let (true_reserve, false_reserve) = Self::get_reserves(env);
        true_reserve.checked_mul(false_reserve).expect("k overflow")
    }

    pub fn get_deposits(env: Env) -> (i128, i128) {
        let store = env.storage().persistent();
        let true_deposits: i128 = store.get(&StorageKey::TrueDeposits).unwrap();
        let false_deposits: i128 = store.get(&StorageKey::FalseDeposits).unwrap();
        (true_deposits, false_deposits)
    }
}

mod test;