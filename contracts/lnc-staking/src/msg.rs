use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin wallet (supplied by the owner at deploy — never invented).
    pub admin: String,
    /// Staking token, i.e. LNC:
    /// terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm
    pub stake_token: String,
}

/// Binary payload sent inside a CW20 `Send` to this contract.
/// Exactly one field must be set:
/// * `{"stake": {}}` — stake the sent tokens. Only accepted when the
///   sending token IS the staking token (LNC).
/// * `{"fund": {}}` — fund the reward pool for the sending token. The
///   token must already be registered as a reward (AddReward first, or
///   the staking token itself, pre-registered at deploy) so funds can
///   never arrive unconfigured.
#[cw_serde]
#[derive(Default)]
pub struct FundReceiveMsg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stake: Option<EmptyMsg>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fund: Option<EmptyMsg>,
}

#[cw_serde]
pub struct EmptyMsg {}

// Standard CW20 receive wrapper (defined locally to avoid cw20 crate
// version coupling — wire format is identical).
#[cw_serde]
pub struct Cw20ReceiveMsg {
    pub sender: String,
    pub amount: Uint128,
    pub msg: cosmwasm_std::Binary,
}

/// CW20 transfer message shape (for paying out principal and rewards).
#[cw_serde]
pub struct Cw20TransferMsg {
    pub recipient: String,
    pub amount: Uint128,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// CW20 hook: stake or fund (see FundReceiveMsg).
    Receive(Cw20ReceiveMsg),
    /// Return staked principal. NEVER blocked by pause. Accrued rewards
    /// stay banked and remain claimable afterwards.
    Unstake { amount: Uint128 },
    /// Claim banked rewards for one reward token.
    Claim { reward_token: String },
    /// Claim banked rewards for all reward tokens.
    ClaimAll {},
    // --- admin only (single wallet, transferable in two steps) ---
    /// Register an additional reward token (rate starts at 0).
    AddReward { reward_token: String },
    /// rate_per_token_per_day = raw reward units per 1 WHOLE staked token
    /// per day. Starts at 0 for every reward, including LNC at deploy.
    SetRewardRate { reward_token: String, rate_per_token_per_day: Uint128 },
    /// Freeze/unfreeze one reward's accrual. Pausing first accrues up to
    /// now, so no rewards leak across the paused span on resume.
    PauseReward { reward_token: String, paused: bool },
    /// Global pause: blocks staking and claiming only.
    PauseAll {},
    Unpause {},
    /// Step 1 of the admin transfer: current admin names a successor.
    TransferAdmin { new_admin: String },
    /// Step 2: the pending admin accepts. Old admin loses rights at once.
    AcceptAdmin {},
    /// Withdraw UNPROMISED reward-pool funds only. Capped at pool_balance:
    /// staked principal and banked claims can never be withdrawn.
    WithdrawReward { reward_token: String, amount: Uint128, to: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(PositionResponse)]
    Position { owner: String },
    #[returns(RewardsResponse)]
    Rewards {},
    #[returns(PoolBalancesResponse)]
    PoolBalances {},
    #[returns(PendingResponse)]
    Pending { owner: String },
}

#[cw_serde]
pub struct ConfigResponse {
    pub admin: Addr,
    pub pending_admin: Option<Addr>,
    pub stake_token: Addr,
    pub paused: bool,
    pub total_staked: Uint128,
}

#[cw_serde]
pub struct PendingEntry {
    pub reward_token: Addr,
    pub amount: Uint128,
}

#[cw_serde]
pub struct PositionResponse {
    pub owner: Addr,
    pub amount: Uint128,
    pub pending: Vec<PendingEntry>,
}

#[cw_serde]
pub struct RewardInfo {
    pub reward_token: Addr,
    pub rate_per_token_per_day: Uint128,
    pub enabled: bool,
    pub pool_balance: Uint128,
}

#[cw_serde]
pub struct RewardsResponse {
    pub rewards: Vec<RewardInfo>,
}

#[cw_serde]
pub struct PoolBalanceEntry {
    pub reward_token: Addr,
    pub pool_balance: Uint128,
}

#[cw_serde]
pub struct PoolBalancesResponse {
    pub pools: Vec<PoolBalanceEntry>,
}

#[cw_serde]
pub struct PendingResponse {
    pub owner: Addr,
    pub pending: Vec<PendingEntry>,
}
