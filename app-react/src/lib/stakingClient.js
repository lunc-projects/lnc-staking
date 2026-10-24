// ─────────────────────────────────────────────────────────────
// ISOLATED LNC staking contract client.
// Every interaction with the LNC staking contract lives here, coded
// against the contract API in contracts/lnc-staking/src/msg.rs:
//   Queries:  Config{}, Position{owner}, Rewards{}, PoolBalances{},
//             Pending{owner}
//   Stake:    CW20 Send of LNC into the contract, hook {"stake":{}}
//   Unstake:  Unstake{amount} — never blocked by pause
//   Claim:    Claim{reward_token} / ClaimAll{}
//   Funding:  CW20 Send of a registered reward token, hook {"fund":{}}
//   Admin:    AddReward{reward_token},
//             SetRewardRate{reward_token,rate_per_token_per_day}
// If message names shift, adjust ONLY this file.
// ─────────────────────────────────────────────────────────────
import { CONTRACT_ADDRESS, LNC } from '../config.js';
import { lcdSmart, b64 } from './lcd.js';

const AUTO_FEE = 'auto';

// ── Queries (LCD reads, no wallet needed) ──
export async function queryConfig() {
  return lcdSmart(CONTRACT_ADDRESS, { config: {} });
}
export async function queryPosition(owner) {
  return lcdSmart(CONTRACT_ADDRESS, { position: { owner } });
}
// CW20 balance of any token for a wallet — used to show how much LNC the
// connected wallet holds and can stake.
export async function queryTokenBalance(token, owner) {
  const out = await lcdSmart(token, { balance: { address: owner } });
  return out?.balance ?? '0';
}
export async function queryRewards() {
  const out = await lcdSmart(CONTRACT_ADDRESS, { rewards: {} });
  return Array.isArray(out) ? out : out?.rewards || [];
}
export async function queryPoolBalances() {
  const out = await lcdSmart(CONTRACT_ADDRESS, { pool_balances: {} });
  return Array.isArray(out) ? out : out?.pools || [];
}

// ── Executes (signed by the connected wallet via Cosmos Kit) ──
async function exec(signingClient, sender, contract, msg) {
  if (!signingClient) throw new Error('Connect a wallet first.');
  return signingClient.execute(sender, contract, msg, AUTO_FEE);
}

export function stakeLnc(signingClient, sender, amountBaseUnits) {
  // CW20 Send of LNC into the contract, hook {"stake":{}}.
  return exec(signingClient, sender, LNC.address, {
    send: { contract: CONTRACT_ADDRESS, amount: String(amountBaseUnits), msg: b64({ stake: {} }) },
  });
}
export function unstake(signingClient, sender, amountBaseUnits) {
  return exec(signingClient, sender, CONTRACT_ADDRESS, {
    unstake: { amount: String(amountBaseUnits) },
  });
}
export function claimAll(signingClient, sender) {
  return exec(signingClient, sender, CONTRACT_ADDRESS, { claim_all: {} });
}

// ── Admin ──
export function adminSetRewardRate(signingClient, sender, rewardToken, rateBaseUnitsPerDay) {
  return exec(signingClient, sender, CONTRACT_ADDRESS, {
    set_reward_rate: { reward_token: rewardToken, rate_per_token_per_day: String(rateBaseUnitsPerDay) },
  });
}
export function adminFundPool(signingClient, sender, rewardToken, amountBaseUnits) {
  // CW20 Send of the reward token into the contract, hook {"fund":{}}.
  // The token must already be registered (LNC is registered from deploy).
  return exec(signingClient, sender, rewardToken, {
    send: { contract: CONTRACT_ADDRESS, amount: String(amountBaseUnits), msg: b64({ fund: {} }) },
  });
}
export function adminAddReward(signingClient, sender, rewardToken) {
  return exec(signingClient, sender, CONTRACT_ADDRESS, {
    add_reward: { reward_token: rewardToken },
  });
}
