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

#[derive(Clone)]
#[contracttype]
pub struct Bets {
    pub bettor: Address,
    pub amount: i128,
    pub shares: i128,
    pub bet_on_true: bool,
    pub claimed: bool,
}

#[contracttype]
pub enum StorageKey {
    Oracle,
    Token,
    TrueTotal,
    FalseTotal,
    Market,
    State,
    Bets(Address),
    LiquidityParameter,
    TrueShares,
    FalseShares,
}

#[contractimpl]
impl SoroMarket {
    pub fn setup(env: Env, oracle: Address, token: Address, market: String, liquidity_param: i128) {
        let store = env.storage().persistent();
        if store.get::<_, Address>(&StorageKey::Oracle).is_some() {
            panic!("Market already setup");
        }
        let zero: i128 = 0;
        store.set(&StorageKey::Oracle, &oracle);
        store.set(&StorageKey::Token, &token);
        store.set(&StorageKey::TrueTotal, &zero);
        store.set(&StorageKey::FalseTotal, &zero);
        store.set(&StorageKey::TrueShares, &zero);
        store.set(&StorageKey::FalseShares, &zero);
        store.set(&StorageKey::Market, &market);
        store.set(&StorageKey::State, &Outcome::Undecided);
        store.set(&StorageKey::LiquidityParameter, &liquidity_param);
    }

    pub fn bet(env: Env, user: Address, amount: i128, bet_on_true: bool) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        assert_eq!(state, Outcome::Undecided, "Market not live");
        if amount <= 0 {
            panic!("Must send positive amount to bet");
        }
        if store.get::<_, Bets>(&StorageKey::Bets(user.clone())).is_some() {
            panic!("Already bet");
        }
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let liquidity_param: i128 = store.get(&StorageKey::LiquidityParameter).unwrap();
        let shares_to_buy = Self::calculate_shares_for_cost(amount, bet_on_true, true_shares, false_shares, liquidity_param);
        let token: Address = store.get(&StorageKey::Token).unwrap();
        TokenClient::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &user,
            &env.current_contract_address(),
            &amount,
        );
        if bet_on_true {
            let mut true_total: i128 = store.get(&StorageKey::TrueTotal).unwrap();
            let new_true_shares = true_shares + shares_to_buy;
            true_total += amount;
            store.set(&StorageKey::TrueTotal, &true_total);
            store.set(&StorageKey::TrueShares, &new_true_shares);
        } else {
            let mut false_total: i128 = store.get(&StorageKey::FalseTotal).unwrap();
            let new_false_shares = false_shares + shares_to_buy;
            false_total += amount;
            store.set(&StorageKey::FalseTotal, &false_total);
            store.set(&StorageKey::FalseShares, &new_false_shares);
        }
        let entry = Bets {
            bettor: user.clone(),
            amount,
            shares: shares_to_buy,
            bet_on_true,
            claimed: false,
        };
        store.set(&StorageKey::Bets(user), &entry);
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
        let mut user_bet: Bets = store.get(&StorageKey::Bets(user.clone())).unwrap();
        assert!(!user_bet.claimed, "Already claimed");
        let true_total: i128 = store.get(&StorageKey::TrueTotal).unwrap();
        let false_total: i128 = store.get(&StorageKey::FalseTotal).unwrap();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let mut winnings: i128 = 0;
        const SCALE: i128 = 1_000_000;
        if user_bet.bet_on_true && state == Outcome::TrueOutcome {
            let total_pool = true_total + false_total;
            let share = SCALE * user_bet.shares / true_shares;
            winnings = total_pool * share / SCALE;
        } else if !user_bet.bet_on_true && state == Outcome::FalseOutcome {
            let total_pool = true_total + false_total;
            let share = SCALE * user_bet.shares / false_shares;
            winnings = total_pool * share / SCALE;
        }
        if winnings > 0 {
            user_bet.claimed = true;
            store.set(&StorageKey::Bets(user.clone()), &user_bet);
            let token: Address = store.get(&StorageKey::Token).unwrap();
            TokenClient::new(&env, &token).transfer(
                &env.current_contract_address(),
                &user,
                &winnings,
            );
        }
    }

    fn calculate_shares_for_cost(cost: i128, bet_on_true: bool, true_shares: i128, false_shares: i128, liquidity_param: i128) -> i128 {     
        if true_shares == 0 && false_shares == 0 {
            return cost;
        }
        let (current_shares, other_shares) = if bet_on_true {
            (true_shares, false_shares)
        } else {
            (false_shares, true_shares)
        };
        Self::calculate_lsmr_shares(cost, current_shares, other_shares, liquidity_param)
    }
    
    fn calculate_lsmr_shares(cost: i128, current_shares: i128, other_shares: i128, liquidity_param: i128) -> i128 {
        const SCALE: i128 = 1_000_000;
        let total_shares = current_shares + other_shares;
        if total_shares == 0 {
            return cost;
        }
        let current_prob = SCALE * current_shares / total_shares;
        let price_per_share = current_prob + (SCALE - current_prob) * liquidity_param / SCALE;
        if price_per_share == 0 {
            return cost;
        }
        cost * SCALE / price_per_share
    }
    
    pub fn get_price_for_shares(env: Env, shares: i128, bet_on_true: bool) -> i128 {
        let store = env.storage().persistent();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let liquidity_param: i128 = store.get(&StorageKey::LiquidityParameter).unwrap();
        Self::calculate_price_for_shares(shares, bet_on_true, true_shares, false_shares, liquidity_param)
    }
    
    fn calculate_price_for_shares(shares: i128, bet_on_true: bool, true_shares: i128, false_shares: i128, liquidity_param: i128) -> i128 {
        const SCALE: i128 = 1_000_000;
        if true_shares == 0 && false_shares == 0 {
            return shares;
        }
        let (current_shares, other_shares) = if bet_on_true {
            (true_shares, false_shares)
        } else {
            (false_shares, true_shares)
        };
        let total_shares = current_shares + other_shares;
        if total_shares == 0 {
            return shares;
        }
        let current_prob = SCALE * current_shares / total_shares;
        let price_per_share = current_prob + (SCALE - current_prob) * liquidity_param / SCALE;
        shares * price_per_share / SCALE
    }

    pub fn get_market_info(env: Env) -> (i128, i128, i128, i128) {
        let store = env.storage().persistent();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let true_total: i128 = store.get(&StorageKey::TrueTotal).unwrap();
        let false_total: i128 = store.get(&StorageKey::FalseTotal).unwrap();
        (true_shares, false_shares, true_total, false_total)
    }

    pub fn get_market_description(env: Env) -> String {
        let store = env.storage().persistent();
        store.get(&StorageKey::Market).unwrap()
    }

    pub fn get_market_state(env: Env) -> Outcome {
        let store = env.storage().persistent();
        store.get(&StorageKey::State).unwrap()
    }

    pub fn get_user_bet(env: Env, user: Address) -> Option<Bets> {
        let store = env.storage().persistent();
        store.get(&StorageKey::Bets(user))
    }

    pub fn get_current_probabilities(env: Env) -> (i128, i128) {
        const SCALE: i128 = 1_000_000;
        let store = env.storage().persistent();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        if true_shares == 0 && false_shares == 0 {
            return (SCALE / 2, SCALE / 2); // 50/50 for empty market
        }
        let total_shares = true_shares + false_shares;
        let true_prob = SCALE * true_shares / total_shares;
        let false_prob = SCALE - true_prob;
        (true_prob, false_prob)
    }

    pub fn get_shares_for_cost(env: Env, cost: i128, bet_on_true: bool) -> i128 {
        let store = env.storage().persistent();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let liquidity_param: i128 = store.get(&StorageKey::LiquidityParameter).unwrap();
        Self::calculate_shares_for_cost(cost, bet_on_true, true_shares, false_shares, liquidity_param)
    }

    pub fn get_oracle(env: Env) -> Address {
        let store = env.storage().persistent();
        store.get(&StorageKey::Oracle).unwrap()
    }

    pub fn get_token(env: Env) -> Address {
        let store = env.storage().persistent();
        store.get(&StorageKey::Token).unwrap()
    }

    pub fn get_liquidity_parameter(env: Env) -> i128 {
        let store = env.storage().persistent();
        store.get(&StorageKey::LiquidityParameter).unwrap()
    }
}

mod test;
