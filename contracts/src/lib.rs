#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Env, Address, Vec, Symbol};

const DECIMALS: i128 = 1_000_000; // USDC-like 6 decimals
const MIN_PRICE: i128 = 10_000; // $0.01
const TOTAL_PRICE_SUM: i128 = 990_000; // $0.99 (includes spread @ 1%)
const MAX_BETTORS_PER_MARKET: u32 = 1000;

fn key_admin() -> Symbol { symbol_short!("ADMIN") }
fn key_market_counter() -> Symbol { symbol_short!("MKT_CNT") }
fn key_bet_counter() -> Symbol { symbol_short!("BET_CNT") }
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
    pub bettor_count: u32,
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
pub struct Bet {
    pub id: u64,
    pub bettor: Address,
    pub market_id: u64,
    pub outcome: u32, // 0 = home, 1 = draw, 2 = away
    pub amount: i128, // locked USD amount (6 decimals)
    pub price: i128,  // odds at time of bet (6 decimals)
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
        env.storage().persistent().set(&key_bet_counter(), &0u64);
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
    ) -> u64 {
        Self::require_admin(&env, &admin);
        Self::validate_odds(odds_home, odds_draw, odds_away);
        let mut counter: u64 = env.storage().persistent().get(&key_market_counter()).unwrap_or(0u64);
        counter += 1;
        env.storage().persistent().set(&key_market_counter(), &counter);
        let market = Market {
            id: counter,
            title: title.clone(),
            start_time,
            odds_home,
            odds_draw,
            odds_away,
            status: MarketStatus::Active,
            bettor_count: 0u32,
        };
        let market_key = Self::market_key(counter);
        env.storage().persistent().set(&market_key, &market);
        let mkbets_key = Self::market_bets_key(counter);
        let empty_vec: Vec<u64> = Vec::new(&env);
        env.storage().persistent().set(&mkbets_key, &empty_vec);
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
        market.status = MarketStatus::Archived;
        env.storage().persistent().set(&market_key, &market);
    }

    pub fn settle_market(env: Env, admin: Address, market_id: u64, outcome: u32) {
        Self::require_admin(&env, &admin);
        if outcome > 2 { panic!("invalid outcome"); }
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        if market.status != MarketStatus::Active { panic!("market not active"); }
        let mkbets_key = Self::market_bets_key(market_id);
        let bet_ids: Vec<u64> = env.storage().persistent().get(&mkbets_key).unwrap_or(Vec::new(&env));
        let mut total_payouts: i128 = 0i128;
        let bet_ids_len = bet_ids.len();
        let mut i = 0u32;
        while i < bet_ids_len {
            let bet_id: u64 = bet_ids.get(i).unwrap();
            let bet_key = Self::bet_key(bet_id);
            let bet: Bet = env.storage().persistent().get(&bet_key).expect("bet not found");
            if bet.outcome == outcome {
                if bet.price == 0 { panic!("bet price zero"); }
                // payout = amount * DECIMALS / price
                let payout = bet.amount.checked_mul(DECIMALS).expect("mul overflow").checked_div(bet.price).expect("div error");
                total_payouts = total_payouts.checked_add(payout).expect("overflow total payouts");
            }
            i += 1;
        }
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        if total_liq < total_payouts { panic!("insufficient liquidity for payouts"); }
        i = 0;
        while i < bet_ids_len {
            let bet_id: u64 = bet_ids.get(i).unwrap();
            let bet_key = Self::bet_key(bet_id);
            let bet: Bet = env.storage().persistent().get(&bet_key).expect("bet not found");
            env.storage().persistent().remove(&bet_key);
            if bet.outcome == outcome {
                let payout = bet.amount.checked_mul(DECIMALS).unwrap().checked_div(bet.price).unwrap();
                Self::credit_user_balance(&env, &bet.bettor, payout);
                total_liq = total_liq.checked_sub(payout).expect("underflow liq");
            }
            i += 1;
        }
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        market.status = MarketStatus::Settled;
        env.storage().persistent().set(&market_key, &market);
        let empty_vec: Vec<u64> = Vec::new(&env);
        env.storage().persistent().set(&mkbets_key, &empty_vec);
    }

    pub fn provide_liquidity(env: Env, provider: Address, amount: i128) {
        if amount <= 0 { panic!("amount must be positive"); }
        // For now: we simulate token transfer externally. Update internal LP balance and total liquidity.
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
        // Simulate deposit: increase user internal balance and total liquidity
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

    pub fn place_bet(
        env: Env,
        user: Address,
        market_id: u64,
        outcome: u32,
        amount: i128,
    ) {
        if amount <= 0 { panic!("bet amount positive"); }
        if outcome > 2 { panic!("invalid outcome"); }
        let mut user_bal: i128 = env.storage().persistent().get(&Self::user_key(&user)).unwrap_or(0i128);
        if user_bal < amount { panic!("insufficient balance"); }
        let market_key = Self::market_key(market_id);
        let mut market: Market = env.storage().persistent().get(&market_key).expect("market not found");
        if market.status != MarketStatus::Active { panic!("market not active"); }
        if market.bettor_count >= MAX_BETTORS_PER_MARKET { panic!("market bettor cap reached"); }
        let price = match outcome {
            0 => market.odds_home,
            1 => market.odds_draw,
            2 => market.odds_away,
            _ => panic!("invalid outcome"),
        };
        if price < MIN_PRICE { panic!("price below minimum"); }
        user_bal = user_bal.checked_sub(amount).expect("underflow user bal");
        env.storage().persistent().set(&Self::user_key(&user), &user_bal);
        let mut total_liq: i128 = env.storage().persistent().get(&key_total_liquidity()).unwrap_or(0i128);
        total_liq = total_liq.checked_add(amount).expect("overflow total liq");
        env.storage().persistent().set(&key_total_liquidity(), &total_liq);
        let mut bet_counter: u64 = env.storage().persistent().get(&key_bet_counter()).unwrap_or(0u64);
        bet_counter += 1;
        env.storage().persistent().set(&key_bet_counter(), &bet_counter);
        let bet = Bet {
            id: bet_counter,
            bettor: user.clone(),
            market_id,
            outcome,
            amount,
            price,
        };
        let bet_key = Self::bet_key(bet_counter);
        env.storage().persistent().set(&bet_key, &bet);
        let mkbets_key = Self::market_bets_key(market_id);
        let mut bet_ids: Vec<u64> = env.storage().persistent().get(&mkbets_key).unwrap_or(Vec::new(&env));
        bet_ids.push_back(bet_counter);
        env.storage().persistent().set(&mkbets_key, &bet_ids);
        market.bettor_count = market.bettor_count.checked_add(1).expect("overflow bettor count");
        env.storage().persistent().set(&market_key, &market);
    }

    pub fn get_market_bets(env: Env, market_id: u64) -> Vec<Bet> {
        let mkbets_key = Self::market_bets_key(market_id);
        let bet_ids: Vec<u64> = env.storage().persistent().get(&mkbets_key).unwrap_or(Vec::new(&env));
        let mut out: Vec<Bet> = Vec::new(&env);
        let bet_ids_len = bet_ids.len();
        let mut i = 0u32;
        while i < bet_ids_len {
            let bet_id: u64 = bet_ids.get(i).unwrap();
            let bet_key = Self::bet_key(bet_id);
            let bet: Bet = env.storage().persistent().get(&bet_key).expect("bet not found");
            out.push_back(bet);
            i += 1;
        }
        out
    }

    pub fn get_bet(env: Env, bet_id: u64) -> Bet {
        let bet_key = Self::bet_key(bet_id);
        env.storage().persistent().get(&bet_key).expect("bet not found")
    }

    pub fn get_market(env: Env, market_id: u64) -> Market {
        let market_key = Self::market_key(market_id);
        env.storage().persistent().get(&market_key).expect("market not found")
    }

    pub fn get_bettor_count(env: Env, market_id: u64) -> u32 {
        let m: Market = Self::get_market(env.clone(), market_id);
        m.bettor_count
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().persistent().get(&key_admin()).expect("admin not set")
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

    fn bet_key(bet_id: u64) -> (Symbol, u64) {
        (symbol_short!("BET"), bet_id)
    }

    fn market_bets_key(market_id: u64) -> (Symbol, u64) {
        (symbol_short!("MKTBETS"), market_id)
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
}

mod test;