# LNC Staking Rewards — contract + app

**Status: NOT DEPLOYED.** Code is written and tested (16/16 unit tests pass).
Deployment is a separate step that needs the owner's explicit go-ahead.

## What it is

A CosmWasm staking contract for Terra Classic (columbus-5): users stake
**LNC** (CW20 `terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm`,
decimals 6) and earn **LNC rewards** over time from a reward pool the admin
funds. Same security posture as the earlier FUN / PIZZAB×ROTTI rewards builds,
adapted from NFT staking to CW20 token staking.

**Designated admin (owner-supplied, verified on-chain, account 5526839):**

- Contract admin + chain-level admin at deploy:
  `terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69`
- This is the owner's own wallet — the same wallet that admins the FRG
  staking v2 app. The admin-transfer procedure below still applies if this
  ever needs to change, but the starting admin is this wallet.

## Security — the Garuda lesson, built in

- **migrate() REJECTS every migration.** If a Migrate message is ever
  submitted against this contract, the migrate entry point errors, the
  transaction rolls back, and the code stays exactly as deployed. There is
  deliberately no upgrade path: a bug fix means deploying a NEW contract,
  never rewriting this one. This is stronger than the earlier builds'
  silent no-op migrate, and it is the point: the Garuda drain vector
  (an open migrate function swapping in attacker code) cannot exist here.
- **No factory, no shared admin.** One contract, one admin wallet.
- **Admin CANNOT touch staked principal.** `WithdrawReward` is capped at
  the reward pool balance. The contract's LNC balance always equals
  `total_staked + Σ pool_balances + Σ banked claims` (tested).
- **Pause never traps funds.** Pause blocks staking and claiming only;
  unstaking always works, and funding the pool still works while paused.
- **No stuck funds.** Funding a reward token that was never registered is
  rejected, so tokens can never arrive with nowhere to be paid from.
- **Rates, fees, locks start at 0.** Fee 0, lock 0, unpaused at deploy;
  reward rate 0 until the admin sets it. Nothing is invented: the owner
  supplies the rate and funds the pool.

## Accrual formula (rewards-per-share index, lazy checkpointing)

Per reward token, the contract tracks `index_scaled`: cumulative reward per
1 WHOLE staked token, scaled by 1e18.

- On each index advance over `elapsed` seconds:
  `index += rate_per_token_per_day × elapsed / 86400 × 1e18`
- On a user's checkpoint (stake / unstake / claim / rate change):
  `owed = staked_raw × (index − debt) / (1_000_000 × 1e18)`
  where `debt` is the index at the user's last checkpoint.
- `owed` is banked into the user's claimable balance and the same amount
  leaves the reward pool — so the pool always holds UNPROMISED funds only.
- **Rounding:** every division truncates (rounds DOWN). Dust stays in the
  pool; it is never minted and never owed later.
- **Pool-dry rule:** if the advance exceeds the pool, only what the pool
  covers is banked; the excess is forfeited, not carried (debt still
  advances). The admin's job is to keep the pool funded; stakers are never
  promised unbacked rewards.
- Example: stake 10 whole LNC (10_000_000 raw) for 1 full day at a rate of
  1_000_000 raw per whole token per day → exactly 10_000_000 raw (10 LNC).

## Message schema

### Instantiate

```json
{
  "admin": "terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69",
  "stake_token": "terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm"
}
```

The staking token is pre-registered as a reward at rate 0, so admin setup
is just: `SetRewardRate` + fund the pool (see DEPLOY.md).

### Execute

```json
// Stake: send LNC to the contract with this payload (CW20 Send)
{ "stake": {} }
// Fund the reward pool: send ANY registered reward token with this payload
{ "fund": {} }
// Direct executes (from a user wallet)
{ "unstake": { "amount": "1000000" } }
{ "claim": { "reward_token": "terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm" } }
{ "claim_all": {} }
// Admin only
{ "add_reward": { "reward_token": "<cw20 address>" } }
{ "set_reward_rate": { "reward_token": "<cw20>", "rate_per_token_per_day": "1000000" } }
{ "pause_reward": { "reward_token": "<cw20>", "paused": true } }
{ "pause_all": {} }
{ "unpause": {} }
{ "transfer_admin": { "new_admin": "<terra1…>" } }
{ "accept_admin": {} }
{ "withdraw_reward": { "reward_token": "<cw20>", "amount": "1000000", "to": "<terra1…>" } }
```

### Query

```json
{ "config": {} }
{ "position": { "owner": "<terra1…>" } }
{ "rewards": {} }
{ "pool_balances": {} }
{ "pending": { "owner": "<terra1…>" } }
```

`position` / `pending` return pending rewards computed at the queried block
time (read-only — same math as a checkpoint, pool-capped, without writing).

## Admin-transfer procedure (two steps, contract level)

1. Current admin executes `TransferAdmin { new_admin }` → `pending_admin`
   is set. The current admin keeps full rights until acceptance.
2. The pending admin executes `AcceptAdmin {}` → admin switches at once,
   `pending_admin` clears. Anyone else calling `AcceptAdmin` fails.

**Chain-level admin (separate step, optional):** the chain-level contract
admin (set at instantiate time to the same wallet) is changed with a
`MsgUpdateAdmin` transaction from the current chain admin. This changes
who can submit chain-level admin actions — it does NOT change the code
(migrate is rejected regardless of who the admin is).

## Build & test

```bash
# Rust 1.85.1 toolchain (rustup) — from this directory:
cargo test        # 16/16 unit tests
cargo build --release --target wasm32-unknown-unknown
# deployable binary lands at:
#   target/wasm32-unknown-unknown/release/lnc_staking.wasm
# (a copy is kept at contracts/lnc-staking/lnc_staking.wasm)
```

Tests cover: exact stake/claim math, multi-user interleaved staking,
mid-stake rate changes, zero-rate behavior, pause gating (stake + claim
blocked, unstake never blocked), admin-only gating, principal isolation,
migrate rejection, rounding dust truncation, pool-dry capping, the
two-step admin transfer, per-reward pause freezing the index, rejected
unregistered funding / wrong-token staking, claim after full unstake, and
the balance invariant `pool + banked + total_staked + paid == funded + staked`.

## Layout

- `contracts/lnc-staking/` — Rust contract (lib.rs, msg.rs, state.rs,
  contract.rs, error.rs, tests.rs) + built `lnc_staking.wasm` (389,011 bytes,
  local release build — see DEPLOY.md for the optimizer note)
- `app/` — standalone web app skeleton (connect wallet, stake, unstake,
  claim, pending rewards). Placeholder contract address, clearly marked
  NOT DEPLOYED; it sends no transactions until a real address is set.
- `DEPLOY.md` — exact deploy steps for the owner (no keys anywhere).
