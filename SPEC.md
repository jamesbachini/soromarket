Prediction Market Smart Contract Specification (Soroban)
Overview

The project is a sports betting smart contract that allows users to bet on matches with three possible outcomes: Win, Draw, or Lose. Users maintain an internal USD balance within the contract, which can be topped up or withdrawn in USDC. Bets are settled automatically via a central liquidity pool, so users do not need to manually claim winnings.

Users and Balances

Each user has an internal USD balance tracked in a custom ledger within the contract.

Users can deposit USDC into the contract to increase their internal balance and withdraw back to USDC.

When placing bets, the corresponding amount is locked immediately from the user's balance.

Users cannot bet more than their current internal balance.

Liquidity Pool

A central liquidity pool handles all bet payouts.

Any user can provide liquidity to the pool and own a portion of it.

Losing bets remain in the pool, increasing liquidity for other users.

Liquidity providers can withdraw their share, with transfers in USDC simulated via the internal ledger.

Total liquidity is updated automatically as bets are placed and settled.

Markets

A single contract manages all markets/matches (e.g., “Brazil vs England”).

Each market has a unique ID (incrementing counter), metadata (title, start time, odds), and a status flag (Active, Settled, Archived).

Odds for each outcome are stored as integers in USDC-like decimals (e.g., $0.40 = 400000), with total odds summing to $0.99 to provide a spread for liquidity providers.

Markets enforce a maximum of 1000 bettors to avoid gas or computation issues during settlement.

Betting is allowed even after the match is over; odds adjust dynamically in real-time with betting activity, approaching $1 for the likely winner while never dropping below $0.01.

Bets

Bets are placed on a specific outcome with a specified stake amount.

The odds at the time of betting determine potential payout.

Bets are recorded in the market and locked from the user’s balance.

Settlement automatically calculates winnings for all successful bets and increases the winners’ internal balances directly.

Losing bets remain in the liquidity pool.

Settlement

Admin can settle a market by passing in the final outcome (0 = Home Win, 1 = Draw, 2 = Away Win).

The contract loops through all bets for the market (up to the bettor cap) and automatically credits winners’ balances.

Total payouts are verified against the liquidity pool to prevent underfunding.

After settlement, the market status changes to “Settled” and bet records are cleared.

Admin Functions

A single admin is set at contract deployment.

Admin can:

Create new markets with initial odds.

Adjust odds dynamically to reflect ongoing betting activity.

Archive markets if mistakes occur.

Settle markets by specifying the outcome.

Security & Rules

Users cannot bet more than their internal balance.

Odds cannot drop below $0.01.

Total odds for each market always sum to $0.99.

Market bettor cap of 1000 ensures safe settlement loops.

Admin-only actions are restricted to the designated admin address.

Betting after a match is allowed, but odds adjust to reduce incentive for betting on a winning outcome after the fact.

Logic Flow Summary

Deposit/Withdraw: Users deposit USDC into the internal ledger or withdraw it.

Provide Liquidity: Users can add funds to the central pool; withdrawals reduce the pool.

Create Market: Admin creates a market with a title, initial odds, and active status.

Place Bet: User selects an outcome, stakes an amount, and the funds are locked in the internal balance.

Adjust Odds: Odds update dynamically in response to bets to maintain spread.

Settlement: Admin declares the final outcome; winners’ balances are increased, losers’ stakes remain in the pool, and the market is marked Settled.

Archive Market: Admin can mark a market as Archived if needed.