# LNC Staking app skeleton

Static site, no build step: serve `index.html` + `app.js` from any static
host (same pattern as the earlier rewards apps).

- Wallet: Keplr (`window.keplr`), chain `columbus-5`.
- Signing client: CosmJS via esm.sh (pinned 0.32.3).
- Reads: `position { owner }` query (staked + pending rewards).
- Writes: stake via LNC CW20 `Send` with `{"stake":{}}`; `unstake`,
  `claim_all` direct executes.

**NOT DEPLOYED:** `CONTRACT_ADDRESS` in `app.js` is empty. Until the owner
sets it after deploy, the UI shows no balances and refuses every
transaction. See `../DEPLOY.md` step 5.
