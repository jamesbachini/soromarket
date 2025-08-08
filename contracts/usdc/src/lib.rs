// SPDX-License-Identifier: MIT
// Compatible with OpenZeppelin Stellar Soroban Contracts ^0.4.1

/* Note this is a mock USDC contract anyone can mint unlimited tokens */

#![no_std]

use soroban_sdk::{Address, contract, contractimpl, Env, String};
use stellar_macros::default_impl;
use stellar_tokens::fungible::{Base, FungibleToken};

#[contract]
pub struct USDC;

#[contractimpl]
impl USDC {
    pub fn __constructor(e: &Env) {
        Base::set_metadata(e, 18, String::from_str(e, "USDC"), String::from_str(e, "USDC"));
    }

    pub fn mint(e: &Env, account: Address, amount: i128) {
        Base::mint(e, &account, amount);
    }
}

#[default_impl]
#[contractimpl]
impl FungibleToken for USDC {
    type ContractType = Base;

}
