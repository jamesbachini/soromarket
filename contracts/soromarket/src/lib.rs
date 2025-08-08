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
    // kept for compatibility / historical record if you want — unused by logic
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
    LiquidityParameter,
    TrueShares,
    FalseShares,
    UserTrueShares(Address),
    UserFalseShares(Address),
    Claimed(Address),
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

    /// Trade by token amount.
    /// amount > 0 = buy costing `amount` tokens
    /// amount < 0 = sell to receive `-amount` tokens
    pub fn trade(env: Env, user: Address, amount: i128, bet_on_true: bool) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();
        assert_eq!(state, Outcome::Undecided, "Market not live");
        if amount == 0 {
            panic!("Amount must be non-zero");
        }

        let token: Address = store.get(&StorageKey::Token).unwrap();
        let mut true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let mut false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let liquidity_param: i128 = store.get(&StorageKey::LiquidityParameter).unwrap();

        // Choose keys & user balance (type-annotated to avoid ambiguous integers)
        let (mut user_shares, key, total_key, market_shares_key): (i128, StorageKey, StorageKey, StorageKey) =
            if bet_on_true {
                (
                    store
                        .get(&StorageKey::UserTrueShares(user.clone()))
                        .unwrap_or(0_i128),
                    StorageKey::UserTrueShares(user.clone()),
                    StorageKey::TrueTotal,
                    StorageKey::TrueShares,
                )
            } else {
                (
                    store
                        .get(&StorageKey::UserFalseShares(user.clone()))
                        .unwrap_or(0_i128),
                    StorageKey::UserFalseShares(user.clone()),
                    StorageKey::FalseTotal,
                    StorageKey::FalseShares,
                )
            };

        if amount > 0 {
            // BUY: user pays `amount` tokens, receives computed shares.
            let shares = Self::calculate_shares_for_cost(
                amount,
                bet_on_true,
                true_shares,
                false_shares,
                liquidity_param,
            );
            assert!(shares > 0, "Zero shares for this cost");

            // Pull funds from user first (external call).
            TokenClient::new(&env, &token).transfer_from(
                &env.current_contract_address(),
                &user,
                &env.current_contract_address(),
                &amount,
            );

            // Update on-chain accounting AFTER transfer_from succeeded
            user_shares = user_shares.checked_add(shares).expect("user shares overflow");
            if bet_on_true {
                true_shares = true_shares.checked_add(shares).expect("true_shares overflow");
            } else {
                false_shares = false_shares.checked_add(shares).expect("false_shares overflow");
            }
            let mut total: i128 = store.get(&total_key).unwrap();
            total = total.checked_add(amount).expect("total overflow");
            store.set(&total_key, &total);
            store.set(
                &market_shares_key,
                &(if bet_on_true { true_shares } else { false_shares }),
            );
        } else {
            // SELL: amount is negative; payout = -amount tokens requested
            let payout = -amount;
            // Determine how many shares correspond to that payout at current prices.
            let shares_to_sell = Self::calculate_shares_for_cost(
                payout,
                bet_on_true,
                true_shares,
                false_shares,
                liquidity_param,
            );
            assert!(shares_to_sell > 0, "Zero shares for this payout");
            assert!(user_shares >= shares_to_sell, "Not enough shares to sell");

            // Update accounting BEFORE making external transfer to prevent reentrancy drain
            user_shares = user_shares - shares_to_sell;
            if bet_on_true {
                assert!(true_shares >= shares_to_sell, "Insufficient market shares");
                true_shares = true_shares - shares_to_sell;
            } else {
                assert!(false_shares >= shares_to_sell, "Insufficient market shares");
                false_shares = false_shares - shares_to_sell;
            }

            // Deduct payout from the corresponding total pool so settlement pool stays consistent.
            let mut total: i128 = store.get(&total_key).unwrap();
            assert!(total >= payout, "Insufficient pool to pay this sell");
            total = total - payout;

            store.set(&total_key, &total);
            store.set(
                &market_shares_key,
                &(if bet_on_true { true_shares } else { false_shares }),
            );

            // Send payout to seller (external call)
            TokenClient::new(&env, &token).transfer(
                &env.current_contract_address(),
                &user,
                &payout,
            );
        }

        // Persist user's new share balance
        store.set(&key, &user_shares);
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

    /// Claim winnings after settlement.
    /// After paying out, the user's both-side share balances are zeroed to avoid double-claim confusion.
    /// If market not settled, claim is a no-op.
    pub fn claim(env: Env, user: Address) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&StorageKey::State).unwrap();

        // No-op if not settled yet
        if state == Outcome::Undecided {
            return;
        }

        // Prevent double-claim
        let already_claimed: bool = store
            .get(&StorageKey::Claimed(user.clone()))
            .unwrap_or(false);
        assert!(!already_claimed, "Already claimed");

        let true_total: i128 = store.get(&StorageKey::TrueTotal).unwrap();
        let false_total: i128 = store.get(&StorageKey::FalseTotal).unwrap();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let user_true: i128 = store
            .get(&StorageKey::UserTrueShares(user.clone()))
            .unwrap_or(0);
        let user_false: i128 = store
            .get(&StorageKey::UserFalseShares(user.clone()))
            .unwrap_or(0);

        let mut winnings: i128 = 0;

        // Total pool to distribute among winners
        let total_pool = true_total + false_total;

        if state == Outcome::TrueOutcome && user_true > 0 && true_shares > 0 {
            winnings = winnings
                .checked_add(
                    total_pool
                        .checked_mul(user_true)
                        .expect("mul overflow")
                        / true_shares,
                )
                .expect("winnings overflow");
            store.set(&StorageKey::UserTrueShares(user.clone()), &0i128);
        }

        if state == Outcome::FalseOutcome && user_false > 0 && false_shares > 0 {
            winnings = winnings
                .checked_add(
                    total_pool
                        .checked_mul(user_false)
                        .expect("mul overflow")
                        / false_shares,
                )
                .expect("winnings overflow");
            store.set(&StorageKey::UserFalseShares(user.clone()), &0i128);
        }

        // Zero both sides post-claim to simplify state
        store.set(&StorageKey::UserTrueShares(user.clone()), &0i128);
        store.set(&StorageKey::UserFalseShares(user.clone()), &0i128);

        if winnings > 0 {
            let token: Address = store.get(&StorageKey::Token).unwrap();
            // Safe to transfer after internal accounting updates
            TokenClient::new(&env, &token).transfer(
                &env.current_contract_address(),
                &user,
                &winnings,
            );
        }

        // Mark user as claimed to prevent re-claiming
        store.set(&StorageKey::Claimed(user), &true);
    }

    fn calculate_shares_for_cost(
        cost: i128,
        bet_on_true: bool,
        true_shares: i128,
        false_shares: i128,
        liquidity_param: i128,
    ) -> i128 {
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

    fn calculate_lsmr_shares(
        cost: i128,
        current_shares: i128,
        other_shares: i128,
        liquidity_param: i128,
    ) -> i128 {
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
        // shares = cost / price_per_share  (scaled)
        cost.checked_mul(SCALE).expect("mul overflow") / price_per_share
    }

    pub fn get_price_for_shares(env: Env, shares: i128, bet_on_true: bool) -> i128 {
        let store = env.storage().persistent();
        let true_shares: i128 = store.get(&StorageKey::TrueShares).unwrap();
        let false_shares: i128 = store.get(&StorageKey::FalseShares).unwrap();
        let liquidity_param: i128 = store.get(&StorageKey::LiquidityParameter).unwrap();
        Self::calculate_price_for_shares(
            shares,
            bet_on_true,
            true_shares,
            false_shares,
            liquidity_param,
        )
    }

    fn calculate_price_for_shares(
        shares: i128,
        bet_on_true: bool,
        true_shares: i128,
        false_shares: i128,
        liquidity_param: i128,
    ) -> i128 {
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
        shares
            .checked_mul(price_per_share)
            .expect("mul overflow")
            / SCALE
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

    /// New helper giving both user share balances.
    pub fn get_user_shares(env: Env, user: Address) -> (i128, i128) {
        let store = env.storage().persistent();
        let t = store
            .get(&StorageKey::UserTrueShares(user.clone()))
            .unwrap_or(0);
        let f = store
            .get(&StorageKey::UserFalseShares(user))
            .unwrap_or(0);
        (t, f)
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
        Self::calculate_shares_for_cost(
            cost,
            bet_on_true,
            true_shares,
            false_shares,
            liquidity_param,
        )
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