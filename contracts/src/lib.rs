#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Env, Address, Vec, Symbol};

const DECIMALS: i128 = 1_000_000; // USDC-like 6 decimals
const MIN_PRICE: i128 = 10_000; // $0.01
const TOTAL_PRICE_SUM: i128 = 990_000; // $0.99 (includes spread @ 1%)
const MAX_STAKERS_PER_MARKET: u32 = 1000;
const CASHOUT_FEE_PERCENT: i128 = 5; // 5% fee on early cashout

fn key_admin() -> Symbol { symbol_short!("ADMIN") }
fn key_market_counter() -> Symbol { symbol_short!("MKT_CNT") }
fn key_stake_counter() -> Symbol { symbol_short!("STK_CNT") }
fn key_total_liquidity() -> Symbol { symbol_short!("TOT_LIQ") }

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Market {
    pub id: u64,
    pub title: Symbol,
    pub start_time: i64,
    pub odds_home: i128,
    pub odds_draw: i128,
    pub odds_away: i128,
    pub status: MarketStatus,
    pub staker_count: u32,
    pub reserve_home: i128,
    pub reserve_draw: i128,
    pub reserve_away: i128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum MarketStatus {
    Active,
    Settled,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Stake {
    pub id: u64,
    pub staker: Address,
    pub market_id: u64,
    pub outcome: u32, // 0 = home, 1 = draw, 2 = away
    pub amount: i128, // locked USD amount (6 decimals)
    pub price: i128,  // odds at time of stake (6 decimals)
}

#[contract]
pub struct PredictionMarketContract;

#[contractimpl]
impl PredictionMarketContract {

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&key_admin()) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&key_admin(), &admin);
        env.storage().persistent().set(&key_market_counter(), &0u64);
        env.storage().persistent().set(&key_stake_counter(), &0u64);
        env.storage().persistent().set(&key_total_liquidity(), &0i128);
    }

    pub fn create_market(
        env: Env,
        admin: Address,
        title: Symbol,
        start_time: i64,
        odds_home: i128,
        odds_draw: i128,
        odds_away: i128,
        initial_liquidity: i128,
    ) -> u64 {
        Self::require_admin(&env, &admin);
        Self::validate_odds(odds_home, odds_draw, odds_away);
        if initial_liquidity < 0 { panic!("initial liquidity must be non-negative"); }
        let mut counter: u64 = env.storage().persistent().get(&key_market_counter()).unwrap_or(0u64);
        counter += 1;
        env.storage().persistent().set(&key_market_counter(), &counter);
        // Initialize reserves proportional to initial odds using provided liquidity
        let reserve_home = Self::calculate_reserve_from_price(initial_liquidity, odds_home);
        let reserve_draw = Self::calculate_reserve_from_price(initial_liquidity, odds_draw);
        let reserve_away = Self::calculate_reserve_from_price(initial_liquidity, odds_away);
        let market = Market {
            id: counter,
            title: title.clone(),
            start_time,
            odds_home,
            odds_draw,
            odds_away,
            status: MarketStatus::Active,
            staker_count: 0u32,
            reserve_home,
            reserve_draw,
            reserve_away,
        };
        let market_key = Self::market_key(counter);
        env.storage().persistent().set(&market_key, &market);
        let mkstakes_key = Self::market_stakes_key(counter);
        let empty_vec: Vec<u64> = Vec::new(&env);
        env.storage().persistent().set(&mkstakes_key, &empty_vec);
        counter
    }

    pub fn update_odds(
        env: Env,
        admin: Address,
        market_id: u64,
        odds_home: i128,
        odds_draw: i128,
        odds_away: i128,
    ) {
        Self::require_admin(&env, &admin);
        Self::validate_odds(odds_home, odds_draw, odds_away);
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        market.odds_home = odds_home;
        market.odds_draw = odds_draw;
        market.odds_away = odds_away;
        env.storage().persistent().set(&market_key, &market);
    }

    pub fn archive_market(env: Env, admin: Address, market_id: u64) {
        Self::require_admin(&env, &admin);
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        market.reserve_home = 0;
        market.reserve_draw = 0;
        market.reserve_away = 0;
        market.status = MarketStatus::Archived;
        env.storage().persistent().set(&market_key, &market);
    }

    pub fn settle_market(env: Env, admin: Address, market_id: u64, outcome: u32) {
        Self::require_admin(&env, &admin);
        if outcome > 2 { panic!("invalid outcome"); }
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        if market.status != MarketStatus::Active { panic!("market not active"); }
        let mkstakes_key = Self::market_stakes_key(market_id);
        let stake_ids: Vec<u64> = env.storage().persistent().get(&mkstakes_key).unwrap_or(Vec::new(&env));
        let mut total_winning_shares: i128 = 0i128;
        let stake_ids_len = stake_ids.len();
        let mut i = 0u32;
        while i < stake_ids_len {
            let stake_id: u64 = stake_ids.get(i).unwrap();
            let stake_key = Self::stake_key(stake_id);
            let stake: Stake = env.storage().persistent().get(&stake_key).expect("stake not found");
            if stake.outcome == outcome {
                total_winning_shares = total_winning_shares.checked_add(stake.amount).expect("overflow winning shares");
            }
            i += 1;
        }
        let total_payouts = match outcome {
            0 => market.reserve_home,
            1 => market.reserve_draw,
            2 => market.reserve_away,
            _ => panic!("invalid outcome"),
        };
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        if total_liq < total_payouts { panic!("insufficient liquidity for payouts"); }
        i = 0;
        while i < stake_ids_len {
            let stake_id: u64 = stake_ids.get(i).unwrap();
            let stake_key = Self::stake_key(stake_id);
            let stake: Stake = env.storage().persistent().get(&stake_key).expect("stake not found");
            env.storage().persistent().remove(&stake_key);
            if stake.outcome == outcome {
                // payout = (user_shares / total_winning_shares) * total_payouts
                let payout = if total_winning_shares > 0 {
                    stake.amount.checked_mul(total_payouts).expect("mul overflow").checked_div(total_winning_shares).expect("div error")
                } else {
                    stake.amount
                };
                Self::credit_user_balance(&env, &stake.staker, payout);
                total_liq = total_liq.checked_sub(payout).expect("underflow liq");
            }
            i += 1;
        }
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        market.reserve_home = 0;
        market.reserve_draw = 0;
        market.reserve_away = 0;
        market.status = MarketStatus::Settled;
        env.storage().persistent().set(&market_key, &market);
        let empty_vec: Vec<u64> = Vec::new(&env);
        env.storage().persistent().set(&mkstakes_key, &empty_vec);
    }

    pub fn provide_liquidity(env: Env, provider: Address, amount: i128) {
        if amount <= 0 { panic!("amount must be positive"); }
        let mut lp: i128 = env.storage().persistent().get(&Self::lp_key(&provider)).unwrap_or(0i128);
        lp = lp.checked_add(amount).expect("overflow lp balance");
        env.storage().persistent().set(&Self::lp_key(&provider), &lp);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        total_liq = total_liq.checked_add(amount).expect("overflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        // In production integrate actual USDC transfer into this function (transfer_from provider to contract)
    }

    pub fn withdraw_liquidity(env: Env, provider: Address, amount: i128) {
        if amount <= 0 { panic!("amount must be positive"); }
        let mut lp: i128 = env.storage().persistent().get(&Self::lp_key(&provider)).unwrap_or(0i128);
        if lp < amount { panic!("insufficient lp balance"); }
        lp = lp.checked_sub(amount).expect("underflow lp");
        env.storage().persistent().set(&Self::lp_key(&provider), &lp);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        if total_liq < amount { panic!("insufficient total liquidity"); }
        total_liq = total_liq.checked_sub(amount).expect("underflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        // In production integrate actual USDC transfer (transfer contract -> provider)
    }

    pub fn total_liquidity(env: Env) -> i128 {
        env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128)
    }

    pub fn deposit(env: Env, user: Address, amount: i128) {
        if amount <= 0 { panic!("deposit positive"); }
        Self::credit_user_balance(&env, &user, amount);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        total_liq = total_liq.checked_add(amount).expect("overflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        // In production this should transfer USDC from user to contract
    }

    pub fn withdraw(env: Env, user: Address, amount: i128) {
        if amount <= 0 { panic!("withdraw positive"); }
        let mut bal: i128 = env.storage().persistent().get(&Self::user_key(&user)).unwrap_or(0i128);
        if bal < amount { panic!("insufficient balance"); }
        bal = bal.checked_sub(amount).expect("underflow user bal");
        env.storage().persistent().set(&Self::user_key(&user), &bal);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        if total_liq < amount { panic!("insufficient contract liquidity"); }
        total_liq = total_liq.checked_sub(amount).expect("underflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        // In production this should transfer USDC from contract to user
    }

    pub fn get_balance(env: Env, user: Address) -> i128 {
        env.storage().persistent().get(&Self::user_key(&user)).unwrap_or(0i128)
    }

    pub fn place_stake(
        env: Env,
        user: Address,
        market_id: u64,
        outcome: u32,
        amount: i128,
    ) {
        if amount <= 0 { panic!("stake amount positive"); }
        if outcome > 2 { panic!("invalid outcome"); }
        let mut user_bal: i128 = env.storage().persistent().get(&Self::user_key(&user)).unwrap_or(0i128);
        if user_bal < amount { panic!("insufficient balance"); }
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        if market.status != MarketStatus::Active { panic!("market not active"); }
        if market.staker_count >= MAX_STAKERS_PER_MARKET { panic!("market staker cap reached"); }
        let reserve = match outcome {
            0 => market.reserve_home,
            1 => market.reserve_draw,
            2 => market.reserve_away,
            _ => panic!("invalid outcome"),
        };
        let total_reserve = market.reserve_home.checked_add(market.reserve_draw).expect("overflow").checked_add(market.reserve_away).expect("overflow");
        let price = Self::calculate_price_from_reserve(reserve, total_reserve);
        // Calculate shares using CPMM with slippage: shares = reserve * amount / (reserve + amount)
        // This ensures buy-then-sell cannot be profitable
        let shares = if reserve == 0 {
            amount // First buyer gets 1:1
        } else {
            reserve.checked_mul(amount).expect("mul overflow").checked_div(reserve.checked_add(amount).expect("overflow")).expect("div error")
        };
        match outcome {
            0 => market.reserve_home = market.reserve_home.checked_add(amount).expect("overflow reserve"),
            1 => market.reserve_draw = market.reserve_draw.checked_add(amount).expect("overflow reserve"),
            2 => market.reserve_away = market.reserve_away.checked_add(amount).expect("overflow reserve"),
            _ => panic!("invalid outcome"),
        };

        user_bal = user_bal.checked_sub(amount).expect("underflow user bal");
        env.storage().persistent().set(&Self::user_key(&user), &user_bal);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        total_liq = total_liq.checked_add(amount).expect("overflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        let mut stake_counter: u64 = env.storage().persistent().get(&key_stake_counter()).unwrap_or(0u64);
        stake_counter += 1;
        env.storage().persistent().set(&key_stake_counter(), &stake_counter);
        let stake = Stake {
            id: stake_counter,
            staker: user.clone(),
            market_id,
            outcome,
            amount: shares, // store shares
            price,
        };
        let stake_key = Self::stake_key(stake_counter);
        env.storage().persistent().set(&stake_key, &stake);
        let mkstakes_key = Self::market_stakes_key(market_id);
        let mut stake_ids: Vec<u64> = env.storage().persistent().get(&mkstakes_key).unwrap_or(Vec::new(&env));
        stake_ids.push_back(stake_counter);
        env.storage().persistent().set(&mkstakes_key, &stake_ids);
        market.staker_count = market.staker_count.checked_add(1).expect("overflow staker count");
        env.storage().persistent().set(&market_key, &market);
    }

    pub fn get_market_stakes(env: Env, market_id: u64) -> Vec<Stake> {
        let mkstakes_key = Self::market_stakes_key(market_id);
        let stake_ids: Vec<u64> = env.storage().persistent().get(&mkstakes_key).unwrap_or(Vec::new(&env));
        let mut out: Vec<Stake> = Vec::new(&env);
        let stake_ids_len = stake_ids.len();
        let mut i = 0u32;
        while i < stake_ids_len {
            let stake_id: u64 = stake_ids.get(i).unwrap();
            let stake_key = Self::stake_key(stake_id);
            let stake: Stake = env.storage().persistent().get(&stake_key).expect("stake not found");
            out.push_back(stake);
            i += 1;
        }
        out
    }

    pub fn get_stake(env: Env, stake_id: u64) -> Stake {
        let stake_key = Self::stake_key(stake_id);
        env.storage().persistent().get(&stake_key).expect("stake not found")
    }

    pub fn get_market(env: Env, market_id: u64) -> Market {
        let market_key = Self::market_key(market_id);
        env.storage().persistent().get(&market_key).expect("market not found")
    }

    pub fn get_staker_count(env: Env, market_id: u64) -> u32 {
        let m: Market = Self::get_market(env.clone(), market_id);
        m.staker_count
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().persistent().get(&key_admin()).expect("admin not set")
    }

    pub fn get_current_odds(env: Env, market_id: u64) -> (i128, i128, i128) {
        let market_key = Self::market_key(market_id);
        let market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        let total_reserve = market.reserve_home.checked_add(market.reserve_draw).expect("overflow").checked_add(market.reserve_away).expect("overflow");
        if total_reserve == 0 {
            return (market.odds_home, market.odds_draw, market.odds_away);
        }
        let odds_home = Self::calculate_price_from_reserve(market.reserve_home, total_reserve);
        let odds_draw = Self::calculate_price_from_reserve(market.reserve_draw, total_reserve);
        let odds_away = Self::calculate_price_from_reserve(market.reserve_away, total_reserve);
        (odds_home, odds_draw, odds_away)
    }

    pub fn cash_out(env: Env, user: Address, stake_id: u64) {
        let stake_key = Self::stake_key(stake_id);
        let stake: Stake = env.storage().persistent().get(&stake_key).expect("stake not found");
        if stake.staker != user { panic!("unauthorized: not stake owner"); }
        let market_key = Self::market_key(stake.market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        if market.status != MarketStatus::Active { panic!("market not active"); }
        let reserve = match stake.outcome {
            0 => market.reserve_home,
            1 => market.reserve_draw,
            2 => market.reserve_away,
            _ => panic!("invalid outcome"),
        };
        let shares = stake.amount;
        // Calculate payout using CPMM with slippage: payout = shares * reserve / (reserve + shares)
        // This creates symmetric buy/sell mechanics that prevent arbitrage
        let payout_before_fee = if reserve == 0 {
            0
        } else {
            shares.checked_mul(reserve).expect("mul overflow").checked_div(reserve.checked_add(shares).expect("overflow")).expect("div error")
        };
        let fee = payout_before_fee.checked_mul(CASHOUT_FEE_PERCENT).expect("mul overflow").checked_div(100).expect("div error");
        let payout_after_fee = payout_before_fee.checked_sub(fee).expect("underflow payout");
        // Remove the payout amount from the reserve
        match stake.outcome {
            0 => market.reserve_home = market.reserve_home.checked_sub(payout_before_fee).expect("underflow reserve"),
            1 => market.reserve_draw = market.reserve_draw.checked_sub(payout_before_fee).expect("underflow reserve"),
            2 => market.reserve_away = market.reserve_away.checked_sub(payout_before_fee).expect("underflow reserve"),
            _ => panic!("invalid outcome"),
        };
        Self::credit_user_balance(&env, &user, payout_after_fee);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        total_liq = total_liq.checked_sub(payout_after_fee).expect("underflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        env.storage().persistent().remove(&stake_key);
        let mkstakes_key = Self::market_stakes_key(stake.market_id);
        let stake_ids: Vec<u64> = env.storage().persistent().get(&mkstakes_key).unwrap_or(Vec::new(&env));
        let mut new_stake_ids: Vec<u64> = Vec::new(&env);
        let mut i = 0u32;
        while i < stake_ids.len() {
            let id = stake_ids.get(i).unwrap();
            if id != stake_id {
                new_stake_ids.push_back(id);
            }
            i += 1;
        }
        env.storage().persistent().set(&mkstakes_key, &new_stake_ids);
        market.staker_count = market.staker_count.checked_sub(1).unwrap_or(0);
        env.storage().persistent().set(&market_key, &market);
    }

    fn require_admin(env: &Env, who: &Address) {
        let admin: Address = env.storage().persistent().get(&key_admin()).expect("admin not set");
        if admin != *who { panic!("unauthorized: admin only"); }
    }

    fn validate_odds(odds_home: i128, odds_draw: i128, odds_away: i128) {
        if odds_home < MIN_PRICE || odds_draw < MIN_PRICE || odds_away < MIN_PRICE {
            panic!("odds below minimum");
        }
        let sum = odds_home.checked_add(odds_draw).and_then(|s| s.checked_add(odds_away)).expect("overflow sum");
        if sum != TOTAL_PRICE_SUM { panic!("odds must sum to $0.99 (in 6 decimals)"); }
    }

    fn market_key(market_id: u64) -> (Symbol, u64) {
        (symbol_short!("MKT"), market_id)
    }

    fn stake_key(stake_id: u64) -> (Symbol, u64) {
        (symbol_short!("STK"), stake_id)
    }

    fn market_stakes_key(market_id: u64) -> (Symbol, u64) {
        (symbol_short!("MKTSTKS"), market_id)
    }

    fn user_key(user: &Address) -> (Symbol, Address) {
        (symbol_short!("USR"), user.clone())
    }

    fn lp_key(provider: &Address) -> (Symbol, Address) {
        (symbol_short!("LP"), provider.clone())
    }

    fn credit_user_balance(env: &Env, user: &Address, amount: i128) {
        let mut bal: i128 = env.storage().persistent().get(&Self::user_key(user)).unwrap_or(0i128);
        bal = bal.checked_add(amount).expect("overflow credit user bal");
        env.storage().persistent().set(&Self::user_key(user), &bal);
    }

    fn calculate_reserve_from_price(total_reserve: i128, price: i128) -> i128 {
        // reserve = total_reserve * price / DECIMALS
        total_reserve.checked_mul(price).expect("mul overflow").checked_div(DECIMALS).expect("div error")
    }

    fn calculate_price_from_reserve(reserve: i128, total_reserve: i128) -> i128 {
        // price = reserve * DECIMALS / total_reserve
        if total_reserve == 0 { return 0; }
        reserve.checked_mul(DECIMALS).expect("mul overflow").checked_div(total_reserve).expect("div error")
    }
}

mod test;