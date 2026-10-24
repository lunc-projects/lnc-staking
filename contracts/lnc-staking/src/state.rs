use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

/// Index scaling for the rewards-per-share math: 1e18.
pub const SCALE: u128 = 1_000_000_000_000_000_000;
/// Raw units in 1 whole token, dec-6 convention (LNC is dec 6).
pub const WHOLE_TOKEN_RAW: u128 = 1_000_000;
pub const DAY_SECONDS: u64 = 86_400;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
    /// Two-step transfer: set by TransferAdmin, completed by AcceptAdmin.
    pub pending_admin: Option<Addr>,
    /// The staking token (LNC). Also pre-registered as a reward at deploy.
    pub stake_token: Addr,
    /// Global pause: blocks staking and claiming. Unstaking is NEVER
    /// blocked (funds must never be trapped) and funding still works.
    pub paused: bool,
    /// Total staked principal, raw units. Accounting-only: the admin can
    /// NEVER withdraw this — WithdrawReward is capped at pool balances.
    pub total_staked: Uint128,
}

/// One reward stream, keyed by reward token address.
/// `index_scaled` is the cumulative reward per 1 WHOLE staked token,
/// scaled by SCALE. `pool_balance` is UNPROMISED funds only: when a user
/// checkpoints, their accrued share moves out of the pool into their
/// personal banked balance (ACCRUED), so a later WithdrawReward can never
/// take funds already promised to a staker.
#[cw_serde]
pub struct Reward {
    /// Raw reward units paid per 1 WHOLE staked token per day.
    /// Example: staking 10 whole LNC (10_000_000 raw) for 1 full day at a
    /// rate of 1_000_000 earns exactly 10_000_000 raw reward units.
    pub rate_per_token_per_day: Uint128,
    pub enabled: bool,
    pub pool_balance: Uint128,
    pub index_scaled: Uint128,
    pub last_update_seconds: u64,
}

#[cw_serde]
pub struct Position {
    pub amount: Uint128,
}

pub const CONFIG: Item<Config> = Item::new("config");
/// reward token -> Reward
pub const REWARDS: Map<Addr, Reward> = Map::new("rewards");
/// owner -> Position (single staking token, so one position per owner)
pub const POSITIONS: Map<Addr, Position> = Map::new("positions");
/// Banked rewards: (owner, reward token) -> raw amount owed on claim.
pub const ACCRUED: Map<(Addr, Addr), Uint128> = Map::new("accrued");
/// Index debt: (owner, reward token) -> index at the owner's last checkpoint.
pub const DEBT: Map<(Addr, Addr), Uint128> = Map::new("debt");
