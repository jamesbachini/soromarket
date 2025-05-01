#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Env, Map, Address, String};
use sep_41_token::TokenClient;

#[contract]
pub struct SoroMarket;

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum Outcome {
    Undecided,
    TrueOutcome,
    FalseOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Bets {
    pub bettor: Address,
    pub amount: i128,
    pub bet_on_true: bool,
    pub claimed: bool,
}

#[contractimpl]
impl SoroMarket {
    pub fn setup(env: Env, oracle: Address, token: Address, market: String) {
        let store = env.storage().persistent();
        // ensure not already setup
        if store.get::<_, Address>(&symbol_short!("oracle")).is_some() {
            panic!("Market already setup");
        }
        let zero: i128 = 0;
        store.set(&symbol_short!("oracle"), &oracle);
        store.set(&symbol_short!("token"), &token);
        store.set(&symbol_short!("truetotal"), &zero);
        store.set(&symbol_short!("falsetot"), &zero);
        store.set(&symbol_short!("market"), &market);
        store.set(&symbol_short!("state"), &Outcome::Undecided);
        let bets: Map<Address, Bets> = Map::new(&env);
        store.set(&symbol_short!("bets"), &bets);
    }

    pub fn bet(env: Env, user: Address, amount: i128, bet_on_true: bool) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&symbol_short!("state")).unwrap();
        assert_eq!(state, Outcome::Undecided, "Market not live");
        if amount <= 0 { panic!("Must send positive amount to bet"); }
        let mut bets: Map<Address, Bets> = store.get(&symbol_short!("bets")).unwrap();
        if bets.get(user.clone()).is_some() {
            panic!("Already bet");
        }
        let token: Address = store.get(&symbol_short!("token")).unwrap();
        TokenClient::new(&env, &token).transfer_from(
            &env.current_contract_address(),
            &user,
            &env.current_contract_address(),
            &amount,
        );
        if bet_on_true {
            let mut true_total: i128 = store.get(&symbol_short!("truetotal")).unwrap();
            true_total += amount;
            store.set(&symbol_short!("truetotal"), &true_total);
        } else {
            let mut false_total: i128 = store.get(&symbol_short!("falsetot")).unwrap();
            false_total += amount;
            store.set(&symbol_short!("falsetot"), &false_total);
        }
        let entry = Bets {
            bettor: user.clone(),
            amount,
            bet_on_true,
            claimed: false,
        };
        bets.set(user.clone(), entry);
        store.set(&symbol_short!("bets"), &bets);
    }

    pub fn settle(env: Env, oracle: Address, outcome: bool) {
        oracle.require_auth();
        let store = env.storage().persistent();
        let stored_oracle: Address = store.get(&symbol_short!("oracle")).unwrap();
        assert_eq!(oracle, stored_oracle, "Unauthorized");
        let new_state = if outcome {
            Outcome::TrueOutcome
        } else {
            Outcome::FalseOutcome
        };
        store.set(&symbol_short!("state"), &new_state);
    }

    pub fn claim(env: Env, user: Address) {
        user.require_auth();
        let store = env.storage().persistent();
        let state: Outcome = store.get(&symbol_short!("state")).unwrap();
        let mut bets: Map<Address, Bets> = store.get(&symbol_short!("bets")).unwrap();
        let mut user_bet = bets.get(user.clone()).expect("No bet found");
        assert!(!user_bet.claimed, "Already claimed");
        let true_total: i128 = store.get(&symbol_short!("truetotal")).unwrap();
        let false_total: i128 = store.get(&symbol_short!("falsetot")).unwrap();
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
            bets.set(user.clone(), user_bet.clone());
            store.set(&symbol_short!("bets"), &bets);
            let token: Address = store.get(&symbol_short!("token")).unwrap();
            TokenClient::new(&env, &token).transfer(
                &env.current_contract_address(),
                &user,
                &winnings,
            );
        }
    }
}

mod test;
