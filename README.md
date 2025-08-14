# 🧠 Soro.Market – A Soroban Prediction Market Smart Contract

Live site at https://soro.market

This Soroban smart contract implements a decentralized prediction market on the Stellar blockchain. Users can place bets on binary outcomes and a designated oracle finalizes the result.

Payouts are distributed proportionally to the winning side after settlement.

---

## ✨ Features

- ✅ Binary outcome prediction (true/false)
- 🧪 SEP-41 token integration for staking and payouts
- 👥 Multiple bettors can participate with enforced single-bet policy
- 🧠 Oracle-based settlement system
- 💰 Secure claim mechanism for winners
- 🔐 Prevents double betting and double claiming

---

## 🛠 How It Works

1. **Setup**: An oracle and a token are registered. A prediction is defined (e.g., *"James will be the next president of the USA"*).
2. **Betting**: Users place a single bet using the SEP-41 token, choosing either `true` or `false`.
3. **Settlement**: The oracle sets the final outcome.
4. **Claiming**: Winning participants claim their rewards based on the total pool and their contribution.

---

## 📦 Project Structure

- `contracts/soromarket/lib.rs` – Soroban smart contract.
- `contracts/soromarket/test.rs` – Unit tests.

---

## 🧪 Running Tests

Run tests using:

```bash
cargo test
```

All logic is encapsulated in unit tests using mock environments to simulate user behavior.

---

## 🧾 Example Prediction

> **Prediction**: *"James will be the next president of the USA"*  
> Bettor 1 stakes 100 tokens on `true`, Bettor 2 stakes 200 tokens on `false`.  
> If `true` is correct, Bettor 1 claims the entire pool (minus the losing side’s contribution).

---

## 🔒 Security Notes

- **Immutable outcome**: Once settled, the market cannot be altered.
- **Payout control**: Claims are one-time only and only available to winning bettors.
- **Single-bet enforcement**: Ensures fairness and prevents manipulation.

---

## 📄 License

MIT License © 2025

---