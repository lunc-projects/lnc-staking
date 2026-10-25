# Deploying the LNC staking contract — owner steps

**Do not run these steps until the owner explicitly says go.** No seeds,
mnemonics, or private keys are ever asked for, stored, or pasted anywhere
in this process — the owner signs with their own wallet (Keplr / Station).

## What the owner supplies

- Admin wallet: `terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69`
  (contract admin AND chain-level admin at deploy)
- LUNC for gas (store + instantiate transactions; a few LUNC is plenty —
  exact gas depends on network conditions at the time)
- A reward rate (raw LNC per whole staked LNC per day) — **never invented
  here; the owner sets it** — and LNC to fund the reward pool

## Steps

### 1. Build (already done — verify, don't redo blindly)

```bash
cd ~/workspace/lnc-staking
cargo build --release --target wasm32-unknown-unknown
# -> target/wasm32-unknown-unknown/release/lnc_staking.wasm
```

Note: this is a local release build. For a production deploy, the
deterministic standard is the CosmWasm optimizer
(`cosmwasm/workspace-optimizer`), which produces a smaller, reproducible
binary. Either binary instantiates the same code; if the optimizer is
used, record its checksum alongside the code ID.

### 2. Store the code

From the admin wallet (terrad CLI or Station, chain-id `columbus-5`,
LCD `https://terra-classic-lcd.publicnode.com`):

```bash
terrad tx wasm store lnc_staking.wasm \
  --from <admin-key-name> --chain-id columbus-5 \
  --node https://terra-classic-lcd.publicnode.com \
  --gas auto --gas-adjustment 1.3
```

Record the returned `code_id` and the on-chain `data_hash`, and compare
the `data_hash` against the built binary before going further.

### 3. Instantiate

```json
{
  "admin": "terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69",
  "stake_token": "terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm"
}
```

```bash
terrad tx wasm instantiate <code_id> '<that JSON>' \
  --admin terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69 \
  --from <admin-key-name> --chain-id columbus-5 \
  --label "LNC staking rewards" \
  --node https://terra-classic-lcd.publicnode.com \
  --gas auto --gas-adjustment 1.3
```

Record the contract address. Verify on-chain afterwards:
`config` query shows the admin + stake token above, `rewards` shows LNC
at rate 0, `total_staked` 0, not paused, and (v0.2.0) `min_stake`
3,000,000,000,000 raw units (3,000,000 whole LNC). The instantiate JSON
may omit `min_stake` — it defaults to that value; pass it explicitly
only to start with a different floor.

### 4. Set the rate and fund the pool (admin only, in this order)

1. `SetRewardRate { reward_token: <LNC>, rate_per_token_per_day: <owner's rate> }`
2. Send LNC to the contract with payload `{ "fund": {} }` (CW20 Send) —
   this credits the reward pool. Funding before setting the rate is also
   fine; what matters is that the pool holds funds before stakers arrive.

### 5. Wire the app

Put the contract address in `app/app.js` (`CONTRACT_ADDRESS`), remove the
NOT DEPLOYED banner, and host `app/` as a static site (same pattern as
the earlier rewards apps). The app sends no transactions while the address
is empty.

### 6. Optional, later: chain-level admin change

If the chain-level admin ever needs to move (separate from the contract
admin transfer in README.md), the current chain admin submits a
`MsgUpdateAdmin` for the contract. This does not — and cannot — change
the code: migrate is rejected by design.

## Post-deploy checks (before announcing)

- [ ] `config` query matches the admin + LNC address above
- [ ] Test stake of a small amount from a test wallet, advance, claim —
      amounts match the formula in README.md
- [ ] Test unstake returns the full principal
- [ ] Confirm `migrate` is rejected (submit nothing on mainnet to test
      this — the unit test `migrate_rejected` covers it; do not probe
      with a real Migrate transaction)
- [ ] Confirm the reward pool holds the intended funding before opening
      to the public
