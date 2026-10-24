use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_json, Addr, Binary, CosmosMsg, OwnedDeps, Timestamp, Uint128, WasmMsg,
};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::*;
use crate::state::{ACCRUED, CONFIG, REWARDS};

pub const ADMIN: &str = "admin_wallet";
pub const NEW_ADMIN: &str = "new_admin_wallet";
pub const ALICE: &str = "alice_wallet";
pub const BOB: &str = "bob_wallet";
pub const LNC: &str = "terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm";
pub const OTHER_TOKEN: &str = "terra1other0000000000000000000000000000000";

const DAY: u64 = 86_400;
const T0: u64 = 1_000_000;
/// 1 whole LNC, raw units (dec 6).
const WHOLE: u128 = 1_000_000;

type Deps = OwnedDeps<MockStorage, MockApi, MockQuerier>;

fn env_at(t: u64) -> cosmwasm_std::Env {
    let mut e = mock_env();
    e.block.time = Timestamp::from_seconds(t);
    e
}

fn setup() -> Deps {
    let mut deps = mock_dependencies();
    let msg = InstantiateMsg { admin: ADMIN.into(), stake_token: LNC.into() };
    instantiate(deps.as_mut(), env_at(T0), mock_info(ADMIN, &[]), msg).unwrap();
    deps
}

fn do_exec(
    deps: &mut Deps,
    sender: &str,
    t: u64,
    msg: ExecuteMsg,
) -> Result<cosmwasm_std::Response, ContractError> {
    execute(deps.as_mut(), env_at(t), mock_info(sender, &[]), msg)
}

fn stake_msg() -> Binary {
    cosmwasm_std::to_json_binary(&FundReceiveMsg {
        stake: Some(EmptyMsg {}),
        fund: None,
    })
    .unwrap()
}

fn fund_msg() -> Binary {
    cosmwasm_std::to_json_binary(&FundReceiveMsg {
        stake: None,
        fund: Some(EmptyMsg {}),
    })
    .unwrap()
}

/// Stake `amount` raw LNC for `user` at time `t`.
fn stake(deps: &mut Deps, user: &str, amount: u128, t: u64) {
    do_exec(
        deps,
        LNC,
        t,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: user.into(),
            amount: Uint128::new(amount),
            msg: stake_msg(),
        }),
    )
    .unwrap();
}

/// Fund the LNC reward pool with `amount` raw units at time `t`.
fn fund_lnc(deps: &mut Deps, amount: u128, t: u64) {
    do_exec(
        deps,
        LNC,
        t,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: ADMIN.into(),
            amount: Uint128::new(amount),
            msg: fund_msg(),
        }),
    )
    .unwrap();
}

fn set_rate(deps: &mut Deps, rate: u128, t: u64) {
    do_exec(
        deps,
        ADMIN,
        t,
        ExecuteMsg::SetRewardRate {
            reward_token: LNC.into(),
            rate_per_token_per_day: Uint128::new(rate),
        },
    )
    .unwrap();
}

/// Pending LNC for `owner` at time `t` (includes unbanked index advance).
fn pending(deps: &Deps, owner: &str, t: u64) -> u128 {
    let bin = query(
        deps.as_ref(),
        env_at(t),
        QueryMsg::Pending { owner: owner.into() },
    )
    .unwrap();
    let resp: PendingResponse = from_json(&bin).unwrap();
    resp.pending
        .iter()
        .find(|p| p.reward_token.as_str() == LNC)
        .map(|p| p.amount.u128())
        .unwrap_or(0)
}

fn staked(deps: &Deps, owner: &str, t: u64) -> u128 {
    let bin = query(
        deps.as_ref(),
        env_at(t),
        QueryMsg::Position { owner: owner.into() },
    )
    .unwrap();
    let resp: PositionResponse = from_json(&bin).unwrap();
    resp.amount.u128()
}

fn pool(deps: &Deps) -> u128 {
    let bin = query(deps.as_ref(), env_at(T0), QueryMsg::PoolBalances {}).unwrap();
    let resp: PoolBalancesResponse = from_json(&bin).unwrap();
    resp.pools
        .iter()
        .find(|p| p.reward_token.as_str() == LNC)
        .map(|p| p.pool_balance.u128())
        .unwrap_or(0)
}

/// Assert a response carries exactly one CW20 transfer to `recipient` for `amount` of `token`.
fn assert_transfer(resp: &cosmwasm_std::Response, token: &str, recipient: &str, amount: u128) {
    assert_eq!(resp.messages.len(), 1, "expected exactly one payout message");
    match &resp.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, msg, funds }) => {
            assert_eq!(contract_addr, token);
            assert!(funds.is_empty());
            let t: Cw20TransferMsg = from_json(msg).unwrap();
            assert_eq!(t.recipient, recipient);
            assert_eq!(t.amount.u128(), amount);
        }
        other => panic!("expected CW20 execute, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------

#[test]
fn instantiate_registers_lnc_at_zero_rate() {
    let deps = setup();
    let bin = query(deps.as_ref(), env_at(T0), QueryMsg::Config {}).unwrap();
    let cfg: ConfigResponse = from_json(&bin).unwrap();
    assert_eq!(cfg.admin.as_str(), ADMIN);
    assert_eq!(cfg.stake_token.as_str(), LNC);
    assert_eq!(cfg.pending_admin, None);
    assert!(!cfg.paused);
    assert_eq!(cfg.total_staked.u128(), 0);
    let bin = query(deps.as_ref(), env_at(T0), QueryMsg::Rewards {}).unwrap();
    let rewards: RewardsResponse = from_json(&bin).unwrap();
    assert_eq!(rewards.rewards.len(), 1);
    assert_eq!(rewards.rewards[0].reward_token.as_str(), LNC);
    assert_eq!(rewards.rewards[0].rate_per_token_per_day.u128(), 0);
    assert!(rewards.rewards[0].enabled);
    // Zero rate, zero pool: staking accrues nothing until admin acts.
    assert_eq!(pending(&deps, ALICE, T0), 0);
}

#[test]
fn stake_claim_exact_math() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0); // 1 LNC per whole LNC staked per day
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 10 * WHOLE);
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 10 * WHOLE);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 0);
    assert_eq!(pool(&deps), 90 * WHOLE);
}

#[test]
fn multi_user_interleaved() {
    // Alice stakes 10 at T0; Bob stakes 10 at T0+12h; both claim at T0+24h.
    // Alice: 5 (first half, alone) + 5 (second half) = 10. Bob: 5.
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    stake(&mut deps, BOB, 10 * WHOLE, T0 + DAY / 2);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 10 * WHOLE);
    assert_eq!(pending(&deps, BOB, T0 + DAY), 5 * WHOLE);
    let resp = do_exec(&mut deps, BOB, T0 + DAY, ExecuteMsg::ClaimAll {}).unwrap();
    assert_transfer(&resp, LNC, BOB, 5 * WHOLE);
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 10 * WHOLE);
    assert_eq!(pool(&deps), 85 * WHOLE);
}

#[test]
fn rate_change_mid_stake() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 10 * WHOLE);
    // Double the rate at T0+1d: second day earns 20, not 10.
    set_rate(&mut deps, 2 * WHOLE, T0 + DAY);
    assert_eq!(pending(&deps, ALICE, T0 + 2 * DAY), 30 * WHOLE);
}

#[test]
fn zero_rate_accrues_nothing() {
    let mut deps = setup();
    // No SetRewardRate call: rate stays 0.
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    assert_eq!(pending(&deps, ALICE, T0 + 30 * DAY), 0);
    let err = do_exec(
        &mut deps,
        ALICE,
        T0 + 30 * DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::NothingToClaim {});
}

#[test]
fn pause_blocks_stake_and_claim_but_not_unstake() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    do_exec(&mut deps, ADMIN, T0, ExecuteMsg::PauseAll {}).unwrap();
    let err = do_exec(
        &mut deps,
        LNC,
        T0,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: BOB.into(),
            amount: Uint128::new(WHOLE),
            msg: stake_msg(),
        }),
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Paused {});
    let err = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Paused {});
    // Principal is never trapped: unstake works while paused.
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Unstake { amount: Uint128::new(10 * WHOLE) },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 10 * WHOLE);
    do_exec(&mut deps, ADMIN, T0, ExecuteMsg::Unpause {}).unwrap();
    stake(&mut deps, BOB, WHOLE, T0);
    assert_eq!(staked(&deps, BOB, T0), WHOLE);
}

#[test]
fn admin_only_gating() {
    let mut deps = setup();
    let admin_calls = vec![
        ExecuteMsg::AddReward { reward_token: OTHER_TOKEN.into() },
        ExecuteMsg::SetRewardRate {
            reward_token: LNC.into(),
            rate_per_token_per_day: Uint128::new(WHOLE),
        },
        ExecuteMsg::PauseReward { reward_token: LNC.into(), paused: true },
        ExecuteMsg::PauseAll {},
        ExecuteMsg::Unpause {},
        ExecuteMsg::TransferAdmin { new_admin: NEW_ADMIN.into() },
        ExecuteMsg::WithdrawReward {
            reward_token: LNC.into(),
            amount: Uint128::new(1),
            to: ADMIN.into(),
        },
    ];
    for msg in admin_calls {
        let err = do_exec(&mut deps, ALICE, T0, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }
    // AcceptAdmin with no pending transfer fails for everyone.
    let err = do_exec(&mut deps, NEW_ADMIN, T0, ExecuteMsg::AcceptAdmin {}).unwrap_err();
    assert_eq!(err, ContractError::NoPendingAdmin {});
}

#[test]
fn principal_isolation() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 50 * WHOLE, T0);
    stake(&mut deps, ALICE, 100 * WHOLE, T0);
    // Admin cannot withdraw more than the pool, and cannot reach principal.
    let err = do_exec(
        &mut deps,
        ADMIN,
        T0,
        ExecuteMsg::WithdrawReward {
            reward_token: LNC.into(),
            amount: Uint128::new(51 * WHOLE),
            to: ADMIN.into(),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InsufficientPool {});
    let resp = do_exec(
        &mut deps,
        ADMIN,
        T0,
        ExecuteMsg::WithdrawReward {
            reward_token: LNC.into(),
            amount: Uint128::new(50 * WHOLE),
            to: ADMIN.into(),
        },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ADMIN, 50 * WHOLE);
    // Principal fully intact and returnable afterwards.
    assert_eq!(staked(&deps, ALICE, T0), 100 * WHOLE);
    let bin = query(deps.as_ref(), env_at(T0), QueryMsg::Config {}).unwrap();
    let cfg: ConfigResponse = from_json(&bin).unwrap();
    assert_eq!(cfg.total_staked.u128(), 100 * WHOLE);
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0,
        ExecuteMsg::Unstake { amount: Uint128::new(100 * WHOLE) },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 100 * WHOLE);
}

#[test]
fn migrate_rejected() {
    let mut deps = setup();
    let err = migrate(deps.as_mut(), env_at(T0), MigrateMsg {}).unwrap_err();
    assert_eq!(err, ContractError::MigrationRejected {});
}

#[test]
fn rounding_dust_truncates() {
    // 1 raw unit staked for a day at 1 raw/day/whole-token:
    // exact share = 1 * 1e18 / 1e24 < 1 -> truncates to 0. Pool untouched.
    let mut deps = setup();
    set_rate(&mut deps, 1, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 1, T0);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 0);
    assert_eq!(pool(&deps), 100 * WHOLE);
}

#[test]
fn pool_dry_caps_accrual() {
    // Owed 10 after a day, pool holds only 3: 3 is banked, 7 forfeited
    // (debt advances; no retroactive catch-up when the pool refills).
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 3 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    assert_eq!(pending(&deps, ALICE, T0 + DAY), 3 * WHOLE);
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 3 * WHOLE);
    assert_eq!(pool(&deps), 0);
    fund_lnc(&mut deps, 100 * WHOLE, T0 + DAY);
    // Refill does NOT pay the forfeited 7: only the new day accrues.
    assert_eq!(pending(&deps, ALICE, T0 + 2 * DAY), 10 * WHOLE);
}

#[test]
fn admin_transfer_two_step() {
    let mut deps = setup();
    let err = do_exec(
        &mut deps,
        ALICE,
        T0,
        ExecuteMsg::TransferAdmin { new_admin: NEW_ADMIN.into() },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
    do_exec(
        &mut deps,
        ADMIN,
        T0,
        ExecuteMsg::TransferAdmin { new_admin: NEW_ADMIN.into() },
    )
    .unwrap();
    // Wrong wallet cannot accept.
    let err = do_exec(&mut deps, ALICE, T0, ExecuteMsg::AcceptAdmin {}).unwrap_err();
    assert_eq!(err, ContractError::NotPendingAdmin {});
    // Old admin still holds rights until acceptance.
    set_rate(&mut deps, WHOLE, T0);
    do_exec(&mut deps, NEW_ADMIN, T0, ExecuteMsg::AcceptAdmin {}).unwrap();
    // Rights moved: old admin locked out, new admin in.
    let err = do_exec(
        &mut deps,
        ADMIN,
        T0,
        ExecuteMsg::SetRewardRate {
            reward_token: LNC.into(),
            rate_per_token_per_day: Uint128::new(2 * WHOLE),
        },
    )
    .unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});
    do_exec(
        &mut deps,
        NEW_ADMIN,
        T0,
        ExecuteMsg::SetRewardRate {
            reward_token: LNC.into(),
            rate_per_token_per_day: Uint128::new(2 * WHOLE),
        },
    )
    .unwrap();
    let bin = query(deps.as_ref(), env_at(T0), QueryMsg::Config {}).unwrap();
    let cfg: ConfigResponse = from_json(&bin).unwrap();
    assert_eq!(cfg.admin.as_str(), NEW_ADMIN);
    assert_eq!(cfg.pending_admin, None);
}

#[test]
fn reward_pause_freezes_index() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    // Pause at +12h: 5 banked. A full paused day adds nothing.
    do_exec(
        &mut deps,
        ADMIN,
        T0 + DAY / 2,
        ExecuteMsg::PauseReward { reward_token: LNC.into(), paused: true },
    )
    .unwrap();
    assert_eq!(pending(&deps, ALICE, T0 + DAY / 2 + DAY), 5 * WHOLE);
    do_exec(
        &mut deps,
        ADMIN,
        T0 + DAY / 2 + DAY,
        ExecuteMsg::PauseReward { reward_token: LNC.into(), paused: false },
    )
    .unwrap();
    assert_eq!(pending(&deps, ALICE, T0 + DAY / 2 + DAY + DAY / 2), 10 * WHOLE);
}

#[test]
fn fund_unregistered_and_wrong_stake_token_rejected() {
    let mut deps = setup();
    // Funding a token that was never registered: rejected, no stuck funds.
    let err = do_exec(
        &mut deps,
        OTHER_TOKEN,
        T0,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: ADMIN.into(),
            amount: Uint128::new(WHOLE),
            msg: fund_msg(),
        }),
    )
    .unwrap_err();
    assert_eq!(err, ContractError::UnknownReward {});
    // Staking a non-LNC token: rejected.
    let err = do_exec(
        &mut deps,
        OTHER_TOKEN,
        T0,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: ALICE.into(),
            amount: Uint128::new(WHOLE),
            msg: stake_msg(),
        }),
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidStakeToken {});
    // Malformed receive payloads rejected.
    let err = do_exec(
        &mut deps,
        LNC,
        T0,
        ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: ALICE.into(),
            amount: Uint128::new(WHOLE),
            msg: Binary::from(b"{}"),
        }),
    )
    .unwrap_err();
    assert_eq!(err, ContractError::InvalidReceiveMsg {});
}

#[test]
fn claim_after_full_unstake() {
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 100 * WHOLE, T0);
    stake(&mut deps, ALICE, 10 * WHOLE, T0);
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Unstake { amount: Uint128::new(10 * WHOLE) },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 10 * WHOLE);
    assert_eq!(staked(&deps, ALICE, T0 + DAY), 0);
    // Banked rewards survive the full unstake and stay claimable.
    let resp = do_exec(
        &mut deps,
        ALICE,
        T0 + DAY,
        ExecuteMsg::Claim { reward_token: LNC.into() },
    )
    .unwrap();
    assert_transfer(&resp, LNC, ALICE, 10 * WHOLE);
}

#[test]
fn accounting_invariant() {
    // Contract LNC balance == total_staked + pool + banked + paid_out,
    // and paid_out + pool + banked + total_staked == funded + staked.
    let mut deps = setup();
    set_rate(&mut deps, WHOLE, T0);
    fund_lnc(&mut deps, 200 * WHOLE, T0);
    stake(&mut deps, ALICE, 100 * WHOLE, T0);
    stake(&mut deps, BOB, 50 * WHOLE, T0);
    // Day 1: Alice banks and claims 100; Bob's 50 is still unbanked
    // (pending query shows it, but nothing is banked until he acts).
    let resp = do_exec(&mut deps, ALICE, T0 + DAY, ExecuteMsg::ClaimAll {}).unwrap();
    assert_transfer(&resp, LNC, ALICE, 100 * WHOLE);
    assert_eq!(pending(&deps, BOB, T0 + DAY), 50 * WHOLE);
    let resp = do_exec(&mut deps, BOB, T0 + DAY, ExecuteMsg::ClaimAll {}).unwrap();
    assert_transfer(&resp, LNC, BOB, 50 * WHOLE);
    let cfg = CONFIG.load(&deps.storage).unwrap();
    let reward = REWARDS.load(&deps.storage, Addr::unchecked(LNC)).unwrap();
    let banked_alice = ACCRUED
        .may_load(&deps.storage, (Addr::unchecked(ALICE), Addr::unchecked(LNC)))
        .unwrap()
        .unwrap_or_else(Uint128::zero)
        .u128();
    let banked_bob = ACCRUED
        .may_load(&deps.storage, (Addr::unchecked(BOB), Addr::unchecked(LNC)))
        .unwrap()
        .unwrap_or_else(Uint128::zero)
        .u128();
    // pool 50 + banked 0 + total_staked 150 + paid 150 == funded 200 + staked 150.
    assert_eq!(reward.pool_balance.u128(), 50 * WHOLE);
    assert_eq!(banked_alice + banked_bob, 0);
    assert_eq!(cfg.total_staked.u128(), 150 * WHOLE);
    assert_eq!(50 * WHOLE + 0 + 150 * WHOLE + 150 * WHOLE, 200 * WHOLE + 150 * WHOLE);
}
