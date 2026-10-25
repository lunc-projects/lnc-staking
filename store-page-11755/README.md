# LNC staking — store page (v0.2.0, minimum-stake build)

Stores the v0.2.0 staking code on-chain and instantiates a new contract
from it. This build adds a contract-enforced minimum stake:

- `min_stake` in config, default 3,000,000 whole LNC (about $20 at the
  Oct 2026 Terraport pool ratio — a fixed LNC amount, not a dollar peg).
- Every stake transaction below the minimum is rejected on-chain.
  Unstaking and claiming are unaffected; existing positions are
  unaffected (this is a NEW contract — old positions do not move).
- Admin-adjustable later with `SetMinStake` (0 disables the floor).

Files: `index.html` (page) + `app.bundle.js` (page code, embeds the
expected wasm sha + the code id used for instantiate) +
`lnc_staking.wasm` (the code itself).

Flow: store from this page (page hashes the served wasm in-browser and
refuses upload unless it equals the hardcoded expected sha) -> the
code id constant in `app.bundle.js` is patched to the stored code id ->
instantiate from this page (admin = your wallet, instantiate permission
EVERYBODY). The old store-page/ and store-page-11754/ directories are
the previous builds and are left untouched.
