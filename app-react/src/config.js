// ─────────────────────────────────────────────────────────────
// SINGLE SOURCE OF TRUTH for the LNC Staking app.
// Contract is LIVE (verified on-chain 2026-10-24). Do not change
// these addresses without re-verifying on-chain first.
// ─────────────────────────────────────────────────────────────

export const CHAIN_ID = 'columbus-5';
export const CHAIN_NAME = 'terraclassic'; // Cosmos Kit lookup key (see chainDef.js)
export const LCD_URL = 'https://terra-classic-lcd.publicnode.com';
export const RPC_URL = 'https://terra-classic-rpc.publicnode.com';
export const FINDER = 'https://finder.terraclassic.community/mainnet';

// LNC — the staking token (verified live on-chain).
export const LNC = {
  symbol: 'LNC',
  name: 'LNC',
  address: 'terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm',
  decimals: 6,
};

// LNC staking contract — instantiated 2026-10-24 from code 11753
// (tx 9938975A…), label "LNC staking rewards", admin = owner wallet.
export const CONTRACT_ADDRESS = 'terra1cx4yv0d8vuszdy6fla3ylvaavlp3rtv6ah48xuft3whlu4r5vaqqsnvcqf';

// Contract admin (owner wallet). The admin panel is shown only when
// the connected wallet equals the admin read from the Config query;
// this constant is only a label fallback, never an access check.
export const ADMIN_ADDRESS = 'terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69';

export const DECIMALS = 1_000_000;
export const toRaw = (whole) => String(Math.round(parseFloat(whole) * DECIMALS));
export const toWhole = (raw) => (Number(raw) / DECIMALS).toLocaleString();
export const shortAddr = (a) => (a ? `${a.slice(0, 9)}…${a.slice(-6)}` : '');
