# Soro.Market ⚽  
**Decentralized Prediction Market on Stellar Soroban**

Live site: [https://soro.market](https://soro.market)  


---

## 🧭 Overview
**Soro.Market** is a decentralized sports prediction market built on the **Stellar Soroban** smart contract platform.  
Users can bet on **Win / Draw / Lose** outcomes with internal USD balances backed by USDC.  
The contract is live on **Soroban Testnet**  
`contractId: CA4YXIMAQNIUYAZC3ZRPV5GQSXT4QPXIANL6UYS5CN7FHKKJKDMO7D4M`

---

## ⚙️ Smart Contract
Located in `contracts/src/lib.rs`

### Core Functions
**Admin**
```rust
initialize(admin)
create_market(admin, title, start_time, odds_home, odds_draw, odds_away)
update_odds(admin, market_id, odds_home, odds_draw, odds_away)
settle_market(admin, market_id, outcome)
archive_market(admin, market_id)
````

**Liquidity**

```rust
provide_liquidity(provider, amount)
withdraw_liquidity(provider, amount)
total_liquidity()
```

**Balances**

```rust
deposit(user, amount)
withdraw(user, amount)
get_balance(user)
```

**Staking**

```rust
place_stake(user, market_id, outcome, amount)
get_market_stakes(market_id)
get_stake(stake_id)
```

---

## 🏗️ Frontend

Located in `/docs` (served via GitHub Pages)

| File         | Description                                        |
| ------------ | -------------------------------------------------- |
| `index.html` | Main user interface                                |
| `admin.html` | Admin control panel                                |
| `style.css`  | Styling                                            |
| `app.js`     | Frontend logic (interacting with Soroban contract) |

Theme: **Football (soccer)** — inspired by the **2026 World Cup**.
Frontend is fully functional and interacts directly with the deployed contract (no mocks).

---

## 💡 Core Features

* **Internal USD ledger** with deposits & withdrawals
* **Liquidity pool** backing all markets and payouts
* **Three-way market structure** (Home, Draw, Away)
* **Fixed 1% spread** between odds for liquidity providers
* **Admin settlement** and automated payout distribution

---

## 🧪 Testing

Run contract tests:

```bash
cargo test
```

Tests cover:

* Deposits & withdrawals
* Market creation & staking
* Liquidity & settlement flow

---

## 🗺️ File Structure

```
contracts/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   └── test.rs
docs/
├── index.html
├── admin.html
├── style.css
└── app.js
```

---

**License:** MIT
© 2025 Soro.Market Project


