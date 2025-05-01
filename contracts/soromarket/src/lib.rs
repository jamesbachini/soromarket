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
}

#[contractimpl]
impl SoroMarket {
    pub fn setup(env: Env, oracle: Address, token: Address, market: String) {
        let store = env.storage().persistent();
        if store.get::<_, Address>(&StorageKey::Oracle).is_some() {
            panic!("Market already setup");
        }
        let zero: i128 = 0;
        store.set(&StorageKey::Oracle, &oracle);
        store.set(&StorageKey::Token, &token);
        store.set(&StorageKey::TrueTotal, &zero);
        store.set(&StorageKey::FalseTotal, &zero);
        store.set(&StorageKey::Market, &market);
        store.set(&StorageKey::State, &Outcome::Undecided);
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
        let token: Address = store.get(&StorageKey::Token).unwrap();
        TokenClient::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &user,
            &env.current_contract_address(),
            &amount,
        );
        if bet_on_true {
            let mut true_total: i128 = store.get(&StorageKey::TrueTotal).unwrap();
            true_total += amount;
            store.set(&StorageKey::TrueTotal, &true_total);
        } else {
            let mut false_total: i128 = store.get(&StorageKey::FalseTotal).unwrap();
            false_total += amount;
            store.set(&StorageKey::FalseTotal, &false_total);
        }
        let entry = Bets {
            bettor: user.clone(),
            amount,
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
        let mut winnings: i128 = 0;
        const SCALE: i128 = 1_000_000;
        if user_bet.bet_on_true && state == Outcome::TrueOutcome {
            let share = SCALE * user_bet.amount / true_total;
            winnings = user_bet.amount + (false_total * share / SCALE);
        } else if !user_bet.bet_on_true && state == Outcome::FalseOutcome {
            let share = SCALE * user_bet.amount / false_total;
            winnings = user_bet.amount + (true_total * share / SCALE);
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
}

mod test;
