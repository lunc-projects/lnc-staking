# LNC staking — one-click StoreCode page

## What this is

A standalone static page that stores the LNC staking contract's code on Terra Classic
(chain-id `columbus-5`) with **one wallet approval**. It exists for one reason: governance
proposal 12234 closes **Mon Oct 26, 2026, 6:52 PM ET**, and if it passes, future code uploads
need a governance proposal. Storing the code now preserves the ability to instantiate it
later — instantiation stays open to everybody regardless.

## What it does (and does not do)

- DOES: send exactly one `MsgStoreCode` from the designated wallet, with instantiate
  permission set explicitly to **EVERYBODY**.
- DOES NOT: instantiate a contract, set a reward rate, fund anything, or move staked funds.
- The page refuses to upload from any wallet other than the designated sender
  (`terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69`).
- Before uploading, it downloads `lnc_staking.wasm` from the same directory, hashes it in
  the browser, and requires the hash to equal the expected sha256
  (`9993d22a5fb506945f1658809fcce040a0e318d40c455c1423ca01e53117b4ec`). Upload stays
  disabled on any mismatch.
- After a successful upload it shows the tx hash (Finder link), the new `code_id`, the
  tx-returned checksum, and the on-chain `data_hash` fetched from the LCD — all three
  should match the served file's sha256.

## Files (keep together)

- `index.html` — page + styling
- `app.js` — logic (CosmJS 0.32.4 via esm.sh CDN)
- `lnc_staking.wasm` — the code to store (389,011 bytes; **local release build, NOT the
  CosmWasm optimizer output** — see the caveat below)

## Hosting

Any static host works (GitHub Pages, Vercel, any plain file server). Serve all three files
from the same directory over HTTPS — wallets and `crypto.subtle` require a secure context.
No server-side code, no build step, no secrets on the server.

## How the owner uses it

1. Open the page (desktop browser with Keplr is the supported path).
2. Connect wallet → the page checks the connected address equals the designated sender.
3. Wait for the file check to show ✅.
4. Tap **Upload code to Terra Classic** and approve in the wallet.
5. Record the `code_id` from the result card and confirm the checksums match.

## Honest limitations — read before relying on this

- **NOT live-tested with a real wallet.** The flow is standard CosmJS
  (`SigningCosmWasmClient.connectWithSigner` → `upload(..., "auto", "", { permission: 3 })`),
  and the CDN imports + the upload signature were verified against the published 0.32.4
  bundle — but no real signing, broadcast, or StoreCode transaction was executed from this
  page in testing. The first real use IS the live run; do it with attention, not on autopilot.
- **Keplr is the supported wallet.** Station support is best-effort only: the code attempts
  `window.station.getOfflineSigner` (or `window.station.keplr`) if present, but Station's
  injected API varies by version and was **not verified**. If Station does not work, use
  desktop Keplr. Do not claim Station works until someone tests it.
- **CDN dependency.** The page loads CosmJS from esm.sh at runtime. If esm.sh is unreachable
  or the pinned 0.32.4 files move, the page fails to load — there is no bundled fallback.
  (Pinned versions verified resolving 2026-10-24: `@cosmjs/cosmwasm-stargate@0.32.4`,
  `@cosmjs/stargate@0.32.4`, `@cosmjs/proto-signing@0.32.4`.)
- **Gas price assumption.** The signing client uses `0.15uluna`. If the network rejects the
  tx on fees, that value needs adjusting in `app.js` — it is the one tunable that was not
  verified against live fee conditions.
- **Local release build, not optimizer output.** The served wasm is the local
  `cargo build --release` binary (sha256 above). It instantiates the same code as an
  optimizer build, but the deterministic production standard is the CosmWasm optimizer;
  if the owner later wants a reproducible binary, re-run the optimizer and update both the
  served file and the expected sha256 in `index.html`/`app.js` together.
- **RPC/LCD dependency.** Uses `https://terra-classic-rpc.publicnode.com` (signing) and
  `https://terra-classic-lcd.publicnode.com` (code-info lookup). If either is down, the
  page cannot complete; the design does not fall back to a second endpoint.
- **Instantiate permission is EVERYBODY by design.** That is deliberate (it is what keeps
  instantiation open after 12234), but it means anyone can instantiate this code once stored.
  That is the intended outcome here, not an oversight.
