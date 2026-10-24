# 🍭 LNC Staking app — Luna Classic

Bright candy-themed staking app for LNC on Terra Classic (chain-id `columbus-5`).
Mobile-first, very simple: connect, stake, unstake, claim — plus an owner panel
for adding rewards.

## Files (keep together)

- `index.html` — candy theme page + styling (header logo: `logo.jpg`, shown
  circle-cropped so the red circle reads cleanly; the image itself is untouched)
- `app.bundle.js` — logic, self-contained: CosmJS 0.32.4 bundled locally with
  esbuild (see "Build step" below). **No CDN JavaScript is loaded at runtime.**
- `logo.jpg` — LNC token logo (640×640, used as-is, circle-cropped by CSS)

## What it does

- Wallet: Keplr-first + Station best-effort (`window.station`), chain `columbus-5`.
- Reads: `position { owner }` (staked + pending rewards), `rewards {}` (registered
  reward tokens with rate + pool balance), `config {}` (admin gating).
- Writes: stake via LNC CW20 `Send` with `{"stake":{}}`; `unstake`, `claim_all`
  direct executes.
- Owner panel (visible ONLY when the connected wallet equals the contract's
  admin from the `config` query): set reward rate (`set_reward_rate`), fund the
  pool (CW20 `Send` with `{"fund":{}}`), add a reward token (`add_reward`).
  No pause / admin-transfer / withdraw controls — kept simple on purpose.
- Honest states only: a rate of 0 or an empty pool shows "rewards not started
  yet" — never a fake APY, never invented numbers.

## Build step (self-contained bundle)

The old skeleton loaded CosmJS from esm.sh via `<script type="module">`. That
module graph failed to execute in mobile wallet browsers (page rendered, but no
button handler ever fired). The fix, same as the store page: bundle locally so
the page loads zero third-party JavaScript at runtime.

- Source: `../build/lnc-app.entry.js` (imports rewritten to the npm packages
  `@cosmjs/cosmwasm-stargate@0.32.4` and `@cosmjs/stargate@0.32.4`).
- Deps live in `../build/` (`npm install` there; `build/node_modules/` is never
  committed). Build with esbuild (a `crypto` shim is aliased in because two
  `@cosmjs/crypto` fallbacks `require("crypto")` — both are guarded and never run
  in browsers, which use Web Crypto first):
  ```
  cd ../build && npm install
  ../build/node_modules/.bin/esbuild lnc-app.entry.js --bundle --format=iife \
    --platform=browser --target=es2020 --minify \
    --alias:crypto=./crypto-shim.js --outfile=../app/app.bundle.js
  ```
- `index.html` loads `app.bundle.js` as a plain deferred script. To change app
  logic, edit `../build/lnc-app.entry.js` and re-run the bundle command — never
  hand-edit the bundle.

## NOT LIVE YET

`CONTRACT_ADDRESS` in `../build/lnc-app.entry.js` is empty. Until the owner
instantiates the contract (Step 4 on the store page) and wires the resulting
address in, the app shows the "not live yet" banner, displays no balances, and
refuses every transaction.
