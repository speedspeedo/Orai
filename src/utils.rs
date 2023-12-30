use crate::contract::ORAI;
use crate::{
    msg::ContractStatus,
    state::{Config, Ido, CONFIG_KEY, WHITELIST},
};
use cosmwasm_std::{Addr, Coin, DepsMut, Env, FullDelegation, StdError, StdResult, Storage};
use serde::Deserialize;

pub fn assert_contract_active(storage: &dyn Storage) -> StdResult<()> {
    let config = Config::load(storage)?;
    let active_status = ContractStatus::Active as u8;

    if config.status != active_status {
        return Err(StdError::generic_err("Contract is not active"));
    }

    Ok(())
}

pub fn assert_admin(deps: &DepsMut, address: &String) -> StdResult<()> {
    let canonical_admin = address.clone();
    let config = CONFIG_KEY.load(deps.storage)?;

    if config.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn assert_ido_admin(deps: &DepsMut, address: &String, ido_id: u32) -> StdResult<()> {
    let canonical_admin = address.clone();
    let ido = Ido::load(deps.storage, ido_id)?;

    if ido.admin != canonical_admin {
        return Err(StdError::generic_err("Unauthorized"));
    }

    Ok(())
}

pub fn in_whitelist(storage: &dyn Storage, address: &String, ido_id: u32) -> StdResult<bool> {
    let canonical_address = address.clone();

    let whitelist_status = WHITELIST.may_load(storage, (ido_id, canonical_address))?;

    match whitelist_status {
        Some(value) => Ok(value),
        None => {
            let ido = Ido::load(storage, ido_id)?;
            Ok(ido.shared_whitelist)
        }
    }
}

pub fn sent_funds(coins: &[Coin]) -> StdResult<u128> {
    let mut amount: u128 = 0;

    for coin in coins {
        if coin.denom != "orai" {
            return Err(StdError::generic_err("Unsopported token"));
        }

        amount = amount.checked_add(coin.amount.u128()).unwrap();
    }

    Ok(amount)
}

// Tier

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct FixedDelegationResponse {
    pub _delegation: Option<FixedFullDelegation>,
}

#[derive(Debug, Deserialize)]
pub struct FixedFullDelegation {
    pub delegator: String,
    pub validator: String,
    pub amount: Coin,
    pub can_redelegate: Coin,
    pub accumulated_rewards: Vec<Coin>,
}

impl From<FixedFullDelegation> for FullDelegation {
    fn from(val: FixedFullDelegation) -> Self {
        let found_rewards = val
            .accumulated_rewards
            .into_iter()
            .find(|r| r.denom == ORAI);

        let accumulated_rewards = found_rewards.unwrap_or_else(|| Coin::new(0, ORAI));
        FullDelegation {
            delegator: Addr::unchecked(val.delegator),
            validator: val.validator,
            amount: val.amount,
            can_redelegate: val.can_redelegate,
            accumulated_rewards: vec![accumulated_rewards],
        }
    }
}

pub fn query_delegation(
    deps: &DepsMut,
    env: &Env,
    validator: &String,
) -> StdResult<Option<FullDelegation>> {
    let delegation = deps
        .querier
        .query_delegation(&env.contract.address, validator)?;

    Ok(delegation)
}
