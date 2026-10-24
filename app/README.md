# 🍭 LNC Staking app (live)

React + Vite + Cosmos Kit app, served from this directory on GitHub Pages:
https://lunc-projects.github.io/lnc-staking/app/

- **Wallets (Cosmos Kit modal):** Keplr (extension + mobile via WalletConnect),
  Station (extension), Galaxy Station (custom WalletConnect entry — QR /
  deep link to the wallet app). WalletConnect project ID is the shared
  Reown ID already used by the FRG staking app.
- **Chain:** Terra Classic columbus-5 only (PublicNode RPC/LCD), gas 0.15uluna.
- **Contract (live):** `terra1cx4yv0d8vuszdy6fla3ylvaavlp3rtv6ah48xuft3whlu4r5vaqqsnvcqf`
  (code 11753, admin = owner wallet, stake token LNC).
- **Source:** `../app-react/` — edit there, run `npm run build`, and copy
  `app-react/dist/*` here. Never hand-edit the built files in this directory.
- **Design:** bright pastel candy theme, token logo at `logo.jpg` (used as-is,
  circle-cropped by CSS). No CDN JavaScript at runtime — Vite bundles locally.
- **Honest states:** rate 0 or an empty pool renders as "rewards not started
  yet" / "pool empty" — no APY is ever shown or invented.
