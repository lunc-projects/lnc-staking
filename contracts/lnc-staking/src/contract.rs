use cosmwasm_std::{
    entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdError, StdResult, Uint128, Uint256, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, Cw20ReceiveMsg, Cw20TransferMsg, EmptyMsg, ExecuteMsg, FundReceiveMsg,
    InstantiateMsg, MigrateMsg, PendingEntry, PendingResponse, PoolBalanceEntry,
    PoolBalancesResponse, PositionResponse, QueryMsg, RewardInfo, RewardsResponse,
};
use crate::state::{
    ACCRUED, CONFIG, DAY_SECONDS, DEBT, POSITIONS, REWARDS, Reward, SCALE, WHOLE_TOKEN_RAW,
};

const CONTRACT_NAME: &str = "lnc-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Instantiate
// ---------------------------------------------------------------------------

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = deps.api.addr_validate(&msg.admin)?;
    let stake_token = deps.api.addr_validate(&msg.stake_token)?;
    CONFIG.save(
        deps.storage,
        &crate::state::Config {
            admin: admin.clone(),
            pending_admin: None,
            stake_token: stake_token.clone(),
            paused: false,
            total_staked: Uint128::zero(),
        },
    )?;
    // The staking token is pre-registered as a reward at rate 0, so the
    // admin's only setup steps are SetRewardRate + funding the pool.
    REWARDS.save(
        deps.storage,
        stake_token.clone(),
        &Reward {
            rate_per_token_per_day: Uint128::zero(),
            enabled: true,
            pool_balance: Uint128::zero(),
            index_scaled: Uint128::zero(),
            last_update_seconds: env.block.time.seconds(),
        },
    )?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin)
        .add_attribute("stake_token", stake_token))
}

// ---------------------------------------------------------------------------
// Accrual math (rewards-per-share index, lazy checkpointing)
//
// index_scaled = cumulative reward per 1 WHOLE staked token, scaled by 1e18.
// On each checkpoint the user's share of the index advance since their
// last checkpoint is banked into ACCRUED, and the same amount leaves the
// reward pool — so pool_balance always holds UNPROMISED funds only.
// Rounding: every division truncates (rounds DOWN). Dust stays in the
// pool; it is never minted from nothing and never owed later.
// Pool-dry rule: if the advance exceeds the pool, only what the pool
// covers is banked; the excess is forfeited, not carried (debt still
// advances to the current index). The admin's job is to keep the pool
// funded; stakers are never promised unbacked rewards.
// ---------------------------------------------------------------------------

/// Advance one reward's index to `now`. No-op when disabled or time
/// has not advanced.
fn update_index(
    storage: &mut dyn cosmwasm_std::Storage,
    reward_token: &Addr,
    now: u64,
) -> Result<Reward, ContractError> {
    let mut reward = REWARDS.load(storage, reward_token.clone())?;
    if reward.enabled && now > reward.last_update_seconds {
        let elapsed = now - reward.last_update_seconds;
        let delta = Uint256::from(reward.rate_per_token_per_day.u128())
            * Uint256::from(elapsed)
            * Uint256::from(SCALE)
            / Uint256::from(DAY_SECONDS);
        let delta_u128 = Uint128::try_from(delta)
            .map_err(|_| StdError::generic_err("reward index overflow"))?;
        reward.index_scaled = reward
            .index_scaled
            .checked_add(delta_u128)
            .map_err(|_| StdError::generic_err("reward index overflow"))?;
    }
    reward.last_update_seconds = now;
    REWARDS.save(storage, reward_token.clone(), &reward)?;
    Ok(reward)
}

/// Freeze/unfreeze accrual for one reward. Accrues up to `now` first, so
/// no rewards leak across a paused span when the reward resumes.
fn set_reward_enabled(
    storage: &mut dyn cosmwasm_std::Storage,
    reward_token: &Addr,
    enabled: bool,
    now: u64,
) -> Result<(), ContractError> {
    let mut reward = update_index(storage, reward_token, now)?;
    reward.enabled = enabled;
    reward.last_update_seconds = now;
    REWARDS.save(storage, reward_token.clone(), &reward)?;
    Ok(())
}

/// Bank a user's accrued share for every reward, capped by each pool.
/// Always advances the user's debt to the current index, including for
/// zero positions (so a first stake starts clean, with no retroactive
/// rewards).
fn checkpoint(
    storage: &mut dyn cosmwasm_std::Storage,
    owner: &Addr,
    now: u64,
) -> Result<(), ContractError> {
    let position = POSITIONS
        .may_load(storage, owner.clone())?
        .map(|p| p.amount)
        .unwrap_or_else(Uint128::zero);
    let reward_tokens: Vec<Addr> = REWARDS
        .keys(storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<Addr>>>()?;
    for token in reward_tokens {
        let mut reward = update_index(storage, &token, now)?;
        let debt = DEBT
            .may_load(storage, (owner.clone(), token.clone()))?
            .unwrap_or_else(Uint128::zero);
        if !position.is_zero() {
            let gap = reward
                .index_scaled
                .u128()
                .checked_sub(debt.u128())
                .ok_or_else(|| StdError::generic_err("index debt exceeds index"))?;
            let delta = Uint256::from(position.u128()) * Uint256::from(gap)
                / Uint256::from(WHOLE_TOKEN_RAW * SCALE);
            let delta = Uint128::try_from(delta)
                .map_err(|_| StdError::generic_err("reward accrual overflow"))?;
            // Pool-dry cap: bank only what the pool can back.
            let banked = std::cmp::min(delta, reward.pool_balance);
            if !banked.is_zero() {
                reward.pool_balance = reward.pool_balance.checked_sub(banked)?;
                REWARDS.save(storage, token.clone(), &reward)?;
                let prev = ACCRUED
                    .may_load(storage, (owner.clone(), token.clone()))?
                    .unwrap_or_else(Uint128::zero);
                ACCRUED.save(storage, (owner.clone(), token.clone()), &prev.checked_add(banked)?)?;
            }
        }
        DEBT.save(storage, (owner.clone(), token.clone()), &reward.index_scaled)?;
    }
    Ok(())
}

/// Read-only pending amounts for an owner at `now`, mirroring checkpoint
/// math (including the pool cap) without writing anything.
fn pending_for(deps: Deps, owner: &Addr, now: u64) -> StdResult<Vec<PendingEntry>> {
    let position = POSITIONS
        .may_load(deps.storage, owner.clone())?
        .map(|p| p.amount)
        .unwrap_or_else(Uint128::zero);
    let mut out = Vec::new();
    for item in REWARDS.range(deps.storage, None, None, Order::Ascending) {
        let (token, reward) = item?;
        let mut index = reward.index_scaled;
        if reward.enabled && now > reward.last_update_seconds {
            let elapsed = now - reward.last_update_seconds;
            let delta = Uint256::from(reward.rate_per_token_per_day.u128())
                * Uint256::from(elapsed)
                * Uint256::from(SCALE)
                / Uint256::from(DAY_SECONDS);
            let delta = Uint128::try_from(delta)
                .map_err(|_| StdError::generic_err("reward index overflow"))?;
            index = index.checked_add(delta)?;
        }
        let debt = DEBT
            .may_load(deps.storage, (owner.clone(), token.clone()))?
            .unwrap_or_else(Uint128::zero);
        let mut amount = ACCRUED
            .may_load(deps.storage, (owner.clone(), token.clone()))?
            .unwrap_or_else(Uint128::zero);
        if !position.is_zero() && index.u128() > debt.u128() {
            let gap = index.u128() - debt.u128();
            let delta = Uint256::from(position.u128()) * Uint256::from(gap)
                / Uint256::from(WHOLE_TOKEN_RAW * SCALE);
            let delta = Uint128::try_from(delta)
                .map_err(|_| StdError::generic_err("reward accrual overflow"))?;
            amount = amount.checked_add(std::cmp::min(delta, reward.pool_balance))?;
        }
        out.push(PendingEntry { reward_token: token, amount });
    }
    Ok(out)
}

fn assert_admin(config: &crate::state::Config, sender: &Addr) -> Result<(), ContractError> {
    if config.admin != *sender {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

fn cw20_transfer(token: &Addr, recipient: &str, amount: Uint128) -> StdResult<WasmMsg> {
    Ok(WasmMsg::Execute {
        contract_addr: token.to_string(),
        msg: to_json_binary(&Cw20TransferMsg {
            recipient: recipient.to_string(),
            amount,
        })?,
        funds: vec![],
    })
}

// ---------------------------------------------------------------------------
// Execute
// ---------------------------------------------------------------------------

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(recv) => execute_receive(deps, env, info, recv),
        ExecuteMsg::Unstake { amount } => execute_unstake(deps, env, info, amount),
        ExecuteMsg::Claim { reward_token } => execute_claim(deps, env, info, reward_token),
        ExecuteMsg::ClaimAll {} => execute_claim_all(deps, env, info),
        ExecuteMsg::AddReward { reward_token } => {
            let config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            let token = deps.api.addr_validate(&reward_token)?;
            if REWARDS.has(deps.storage, token.clone()) {
                return Err(ContractError::RewardAlreadyExists {});
            }
            REWARDS.save(
                deps.storage,
                token.clone(),
                &Reward {
                    rate_per_token_per_day: Uint128::zero(),
                    enabled: true,
                    pool_balance: Uint128::zero(),
                    index_scaled: Uint128::zero(),
                    last_update_seconds: env.block.time.seconds(),
                },
            )?;
            Ok(Response::new()
                .add_attribute("action", "add_reward")
                .add_attribute("reward_token", token))
        }
        ExecuteMsg::SetRewardRate { reward_token, rate_per_token_per_day } => {
            let config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            let token = deps.api.addr_validate(&reward_token)?;
            if !REWARDS.has(deps.storage, token.clone()) {
                return Err(ContractError::UnknownReward {});
            }
            let mut reward = update_index(deps.storage, &token, env.block.time.seconds())?;
            reward.rate_per_token_per_day = rate_per_token_per_day;
            REWARDS.save(deps.storage, token.clone(), &reward)?;
            Ok(Response::new()
                .add_attribute("action", "set_reward_rate")
                .add_attribute("reward_token", token)
                .add_attribute("rate_per_token_per_day", rate_per_token_per_day))
        }
        ExecuteMsg::PauseReward { reward_token, paused } => {
            let config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            let token = deps.api.addr_validate(&reward_token)?;
            if !REWARDS.has(deps.storage, token.clone()) {
                return Err(ContractError::UnknownReward {});
            }
            set_reward_enabled(deps.storage, &token, !paused, env.block.time.seconds())?;
            Ok(Response::new()
                .add_attribute("action", "pause_reward")
                .add_attribute("reward_token", token)
                .add_attribute("paused", paused.to_string()))
        }
        ExecuteMsg::PauseAll {} => {
            let mut config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            config.paused = true;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new().add_attribute("action", "pause_all"))
        }
        ExecuteMsg::Unpause {} => {
            let mut config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            config.paused = false;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new().add_attribute("action", "unpause"))
        }
        ExecuteMsg::TransferAdmin { new_admin } => {
            let mut config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            let new_admin = deps.api.addr_validate(&new_admin)?;
            config.pending_admin = Some(new_admin.clone());
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new()
                .add_attribute("action", "transfer_admin")
                .add_attribute("pending_admin", new_admin))
        }
        ExecuteMsg::AcceptAdmin {} => {
            let mut config = CONFIG.load(deps.storage)?;
            let pending = config
                .pending_admin
                .clone()
                .ok_or(ContractError::NoPendingAdmin {})?;
            if pending != info.sender {
                return Err(ContractError::NotPendingAdmin {});
            }
            config.admin = pending.clone();
            config.pending_admin = None;
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new()
                .add_attribute("action", "accept_admin")
                .add_attribute("admin", pending))
        }
        ExecuteMsg::WithdrawReward { reward_token, amount, to } => {
            let config = CONFIG.load(deps.storage)?;
            assert_admin(&config, &info.sender)?;
            let token = deps.api.addr_validate(&reward_token)?;
            if !REWARDS.has(deps.storage, token.clone()) {
                return Err(ContractError::UnknownReward {});
            }
            let mut reward = REWARDS.load(deps.storage, token.clone())?;
            if amount > reward.pool_balance {
                return Err(ContractError::InsufficientPool {});
            }
            reward.pool_balance = reward.pool_balance.checked_sub(amount)?;
            REWARDS.save(deps.storage, token.clone(), &reward)?;
            let to_addr = deps.api.addr_validate(&to)?;
            Ok(Response::new()
                .add_attribute("action", "withdraw_reward")
                .add_attribute("reward_token", token.clone())
                .add_attribute("amount", amount)
                .add_message(cw20_transfer(&token, to_addr.as_str(), amount)?))
        }
    }
}

fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recv: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // In a real CW20 Send, info.sender IS the token contract.
    let token = deps.api.addr_validate(info.sender.as_str())?;
    let sender = deps.api.addr_validate(&recv.sender)?;
    let payload: FundReceiveMsg = cosmwasm_std::from_json(&recv.msg)
        .map_err(|_| ContractError::InvalidReceiveMsg {})?;
    let exactly_one = payload.stake.is_some() ^ payload.fund.is_some();
    if !exactly_one {
        return Err(ContractError::InvalidReceiveMsg {});
    }
    let mut config = CONFIG.load(deps.storage)?;
    let now = env.block.time.seconds();

    if payload.stake.is_some() {
        if config.paused {
            return Err(ContractError::Paused {});
        }
        if token != config.stake_token {
            return Err(ContractError::InvalidStakeToken {});
        }
        if recv.amount.is_zero() {
            return Err(ContractError::InvalidAmount {});
        }
        checkpoint(deps.storage, &sender, now)?;
        let mut position = POSITIONS
            .may_load(deps.storage, sender.clone())?
            .map(|p| p.amount)
            .unwrap_or_else(Uint128::zero);
        position = position.checked_add(recv.amount)?;
        POSITIONS.save(deps.storage, sender.clone(), &crate::state::Position { amount: position })?;
        config.total_staked = config.total_staked.checked_add(recv.amount)?;
        CONFIG.save(deps.storage, &config)?;
        return Ok(Response::new()
            .add_attribute("action", "stake")
            .add_attribute("owner", sender)
            .add_attribute("amount", recv.amount));
    }

    // fund path: token must already be a registered reward — otherwise
    // the funds would arrive with nowhere to be paid from. No pause gate:
    // topping up the pool during a pause is safe and useful.
    if !REWARDS.has(deps.storage, token.clone()) {
        return Err(ContractError::UnknownReward {});
    }
    let mut reward = REWARDS.load(deps.storage, token.clone())?;
    reward.pool_balance = reward.pool_balance.checked_add(recv.amount)?;
    REWARDS.save(deps.storage, token.clone(), &reward)?;
    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("reward_token", token)
        .add_attribute("amount", recv.amount))
}

fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // Deliberately NOT pause-gated: principal must always be retrievable.
    if amount.is_zero() {
        return Err(ContractError::InvalidAmount {});
    }
    let mut config = CONFIG.load(deps.storage)?;
    let now = env.block.time.seconds();
    checkpoint(deps.storage, &info.sender, now)?;
    let mut position = POSITIONS
        .may_load(deps.storage, info.sender.clone())?
        .map(|p| p.amount)
        .unwrap_or_else(Uint128::zero);
    if amount > position {
        return Err(ContractError::InsufficientStake {});
    }
    position = position.checked_sub(amount)?;
    if position.is_zero() {
        POSITIONS.remove(deps.storage, info.sender.clone());
    } else {
        POSITIONS.save(deps.storage, info.sender.clone(), &crate::state::Position { amount: position })?;
    }
    config.total_staked = config.total_staked.checked_sub(amount)?;
    CONFIG.save(deps.storage, &config)?;
    let stake_token = config.stake_token.clone();
    let sender = info.sender.clone();
    Ok(Response::new()
        .add_attribute("action", "unstake")
        .add_attribute("owner", sender.clone())
        .add_attribute("amount", amount)
        .add_message(cw20_transfer(&stake_token, sender.as_str(), amount)?))
}

fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reward_token: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::Paused {});
    }
    let token = deps.api.addr_validate(&reward_token)?;
    if !REWARDS.has(deps.storage, token.clone()) {
        return Err(ContractError::UnknownReward {});
    }
    checkpoint(deps.storage, &info.sender, env.block.time.seconds())?;
    let banked = ACCRUED
        .may_load(deps.storage, (info.sender.clone(), token.clone()))?
        .unwrap_or_else(Uint128::zero);
    if banked.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }
    ACCRUED.save(deps.storage, (info.sender.clone(), token.clone()), &Uint128::zero())?;
    Ok(Response::new()
        .add_attribute("action", "claim")
        .add_attribute("reward_token", token.clone())
        .add_attribute("amount", banked)
        .add_message(cw20_transfer(&token, info.sender.as_str(), banked)?))
}

fn execute_claim_all(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.paused {
        return Err(ContractError::Paused {});
    }
    checkpoint(deps.storage, &info.sender, env.block.time.seconds())?;
    let tokens: Vec<Addr> = REWARDS
        .keys(deps.storage, None, None, Order::Ascending)
        .collect::<StdResult<Vec<Addr>>>()?;
    let mut resp = Response::new().add_attribute("action", "claim_all");
    let mut any = false;
    for token in tokens {
        let banked = ACCRUED
            .may_load(deps.storage, (info.sender.clone(), token.clone()))?
            .unwrap_or_else(Uint128::zero);
        if banked.is_zero() {
            continue;
        }
        ACCRUED.save(deps.storage, (info.sender.clone(), token.clone()), &Uint128::zero())?;
        resp = resp.add_message(cw20_transfer(&token, info.sender.as_str(), banked)?);
        any = true;
    }
    if !any {
        return Err(ContractError::NothingToClaim {});
    }
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&ConfigResponse {
                admin: config.admin,
                pending_admin: config.pending_admin,
                stake_token: config.stake_token,
                paused: config.paused,
                total_staked: config.total_staked,
            })
        }
        QueryMsg::Position { owner } => {
            let owner = deps.api.addr_validate(&owner)?;
            let amount = POSITIONS
                .may_load(deps.storage, owner.clone())?
                .map(|p| p.amount)
                .unwrap_or_else(Uint128::zero);
            let pending = pending_for(deps, &owner, env.block.time.seconds())?;
            to_json_binary(&PositionResponse { owner, amount, pending })
        }
        QueryMsg::Rewards {} => {
            let rewards = REWARDS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|item| {
                    let (token, r) = item?;
                    Ok(RewardInfo {
                        reward_token: token,
                        rate_per_token_per_day: r.rate_per_token_per_day,
                        enabled: r.enabled,
                        pool_balance: r.pool_balance,
                    })
                })
                .collect::<StdResult<Vec<RewardInfo>>>()?;
            to_json_binary(&RewardsResponse { rewards })
        }
        QueryMsg::PoolBalances {} => {
            let pools = REWARDS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|item| {
                    let (token, r) = item?;
                    Ok(PoolBalanceEntry { reward_token: token, pool_balance: r.pool_balance })
                })
                .collect::<StdResult<Vec<PoolBalanceEntry>>>()?;
            to_json_binary(&PoolBalancesResponse { pools })
        }
        QueryMsg::Pending { owner } => {
            let owner = deps.api.addr_validate(&owner)?;
            let pending = pending_for(deps, &owner, env.block.time.seconds())?;
            to_json_binary(&PendingResponse { owner, pending })
        }
    }
}

// ---------------------------------------------------------------------------
// Migrate: REJECTED BY DESIGN.
//
// The Garuda drains worked because a factory's open migrate function let
// an attacker swap staking contracts to malicious code. This contract
// has no factory and a single-wallet admin — but defence in depth says
// the code itself must also refuse to be replaced: if a Migrate message
// is ever submitted against this contract, the migrate entry point
// errors, the transaction rolls back, and the code stays exactly as
// deployed. There is deliberately no upgrade path: a bug fix means
// deploying a NEW contract and moving users, never rewriting this one.
// The admin key can still be transferred (TransferAdmin/AcceptAdmin),
// and the chain-level admin can still be changed (MsgUpdateAdmin) —
// neither of those changes the code.
// ---------------------------------------------------------------------------

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Err(ContractError::MigrationRejected {})
}
