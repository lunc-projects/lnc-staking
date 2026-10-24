use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Contract is paused")]
    Paused {},

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount {},

    #[error("Unknown reward token")]
    UnknownReward {},

    #[error("Reward already registered")]
    RewardAlreadyExists {},

    #[error("Insufficient reward pool balance")]
    InsufficientPool {},

    #[error("Nothing to claim")]
    NothingToClaim {},

    #[error("No position for this owner")]
    NoPosition {},

    #[error("Unstake amount exceeds staked position")]
    InsufficientStake {},

    #[error("Invalid CW20 receive message: exactly one of stake/fund must be set")]
    InvalidReceiveMsg {},

    #[error("This token cannot be staked here")]
    InvalidStakeToken {},

    #[error("No admin transfer is pending")]
    NoPendingAdmin {},

    #[error("Only the pending admin can accept the transfer")]
    NotPendingAdmin {},

    #[error(
        "Migration is rejected by design: this contract can never change its code. \
         Deploy a new contract to upgrade."
    )]
    MigrationRejected {},
}
