use std::convert::TryInto;

#[cfg(not(feature = "library"))]
use crate::band::BandProtocol;
use crate::state;
use cosmwasm_std::entry_point;
use cosmwasm_std::DistributionMsg;
use cosmwasm_std::StakingMsg;
use cosmwasm_std::{
    coin, coins, to_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    ContractStatus, ExecuteMsg, ExecuteResponse, InstantiateMsg, PaymentMethod, QueryMsg,
    QueryResponse, ResponseStatus, SerializedWithdrawals, Whitelist,
};
use crate::utils::{self, assert_admin, assert_contract_active, assert_ido_admin};
use crate::{
    state::{
        Config, Ido, Purchase, UserWithdrawal, ACTIVE_IDOS, ARCHIVED_PURCHASES, CONFIG_KEY,
        IDO_TO_INFO, OWNER_TO_IDOS, PURCHASES, TIER_USER_INFOS, USERINFO, WHITELIST,
        WITHDRAWALS_LIST,
    },
    tier::get_tier,
};
use cosmwasm_std::StdError;

pub const ORAI: &str = "orai";
pub const UNBOUND_LATENCY: u64 = 21 * 24 * 60 * 60;
const CONTRACT_NAME: &str = "crates.io:reward-pay&ment";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ZERO_CODE: i32 = 0;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let admin = msg.admin.unwrap_or(_info.sender.to_string());
    let canonical_admin = admin.to_string();
    let nft_contract = msg.nft_contract.to_string();
    let lock_periods_len = msg.lock_periods.len();

    // Tier Contract
    let deposits = msg.deposits.iter().map(|v| v.u128()).collect::<Vec<_>>();

    if deposits.is_empty() {
        return Err(ContractError::Std(StdError::generic_err(
            "Deposits array is empty",
        )));
    }
    // -------------

    let mut config = Config {
        admin: canonical_admin,
        status: ContractStatus::Active as u8,
        nft_contract,
        lock_periods: msg.lock_periods,
        min_tier: 0,
        validator: msg.validator, // Tier Contract
        usd_deposits: deposits,   // Tier Contract
    };

    let min_tier = config.min_tier();
    config.min_tier = min_tier;

    if lock_periods_len != min_tier as usize {
        return Err(ContractError::Std(StdError::generic_err(&format!(
            "Lock periods array must have {} items",
            min_tier
        ))));
    }

    CONFIG_KEY.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let response = match msg {
        ExecuteMsg::ChangeAdmin { admin, .. } => change_admin(deps, env, info, admin),
        ExecuteMsg::ChangeStatus { status, .. } => change_status(deps, env, info, status),
        ExecuteMsg::StartIdo {
            start_time,
            end_time,
            token_contract: token_contract_addr,
            price,
            total_amount,
            soft_cap,
            tokens_per_tier,
            whitelist,
            payment,
            ..
        } => {
            let mut ido = Ido::default();
            assert_admin(&deps, &info.sender.to_string())?;
            let admin = info.sender.to_string();
            let token_contract = token_contract_addr.to_string();
            ido.admin = admin;
            ido.start_time = start_time;
            ido.end_time = end_time;
            ido.token_contract = token_contract;
            ido.price = price.u128();
            ido.total_tokens_amount = total_amount.u128();
            ido.soft_cap = soft_cap.u128();
            ido.remaining_tokens_per_tier = tokens_per_tier.into_iter().map(|v| v.u128()).collect();

            if let PaymentMethod::Token {
                contract,
                code_hash,
            } = payment
            {
                let payment_token_contract = contract.to_string();
                ido.payment_token_contract = Some(payment_token_contract);
                ido.payment_token_hash = Some(code_hash);
            }

            start_ido(deps, env, info, ido, whitelist)
        }
        ExecuteMsg::BuyTokens {
            amount,
            ido_id,
            viewing_key,
            ..
        } => buy_tokens(deps, env, info, ido_id, amount.u128(), viewing_key),
        ExecuteMsg::WhitelistAdd {
            addresses, ido_id, ..
        } => whitelist_add(deps, env, info, addresses, ido_id),
        ExecuteMsg::WhitelistRemove {
            addresses, ido_id, ..
        } => whitelist_remove(deps, env, info, addresses, ido_id),
        ExecuteMsg::RecvTokens {
            ido_id,
            start,
            limit,
            purchase_indices,
            ..
        } => recv_tokens(deps, env, info, ido_id, start, limit, purchase_indices),
        ExecuteMsg::Withdraw { ido_id, .. } => withdraw(deps, env, info, ido_id),

        // Tier Contract
        ExecuteMsg::Deposit { .. } => try_deposit(deps, env, info),
        ExecuteMsg::WithdrawFromTier { .. } => withdraw_from_tier(deps, env, info),
        ExecuteMsg::Claim {
            recipient,
            start,
            limit,
            ..
        } => try_claim(deps, env, info, recipient, start, limit),
        ExecuteMsg::WithdrawRewards { recipient, .. } => {
            try_withdraw_rewards(deps, env, info, recipient)
        }
        ExecuteMsg::Redelegate {
            validator_address,
            recipient,
            ..
        } => try_redelegate(deps, env, info, validator_address, recipient),
    };

    return response;
}

// #[cfg_attr(not(feature = "library"), entry_point)]
// pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
//     match msg {
//         QueryMsg::Config {} => to_binary(&query_config(deps)?),
//         QueryMsg::TierUserInfo { address } => to_binary(&query_user_info(deps, address)?),
//         QueryMsg::Withdrawals {
//             address,
//             start,
//             limit,
//         } => to_binary(&query_withdrawals(deps, address, start, limit)?),
//     }
// }

fn change_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    admin: String,
) -> Result<Response, ContractError> {
    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    let new_admin = admin.to_string();
    config.admin = new_admin;

    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed admin"))
}

fn change_status(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    status: ContractStatus,
) -> Result<Response, ContractError> {
    assert_admin(&deps, &info.sender.to_string())?;

    let mut config = Config::load(deps.storage)?;
    config.status = status as u8;
    config.save(deps.storage)?;

    Ok(Response::new().add_attribute("action", "changed status"))
}

pub fn get_received_funds(_deps: &DepsMut, info: &MessageInfo) -> Result<Coin, ContractError> {
    match info.funds.get(0) {
        None => return Err(ContractError::Std(StdError::generic_err("No Funds"))),
        Some(received) => {
            /* Amount of tokens received cannot be zero */
            if received.amount.is_zero() {
                return Err(ContractError::Std(StdError::generic_err(
                    "Not Allow Zero Amount",
                )));
            }

            /* Allow to receive only token denomination defined
            on contract instantiation "config.stable_denom" */
            if received.denom.clone() != "orai" {
                return Err(ContractError::Std(StdError::generic_err(
                    "Unsopported token",
                )));
            }

            /* Only one token can be received */
            if info.funds.len() > 1 {
                return Err(ContractError::Std(StdError::generic_err(
                    "Not Allowed Multiple Funds",
                )));
            }
            Ok(received.clone())
        }
    }
}

fn start_ido(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut ido: Ido,
    whitelist: Whitelist,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_admin(&deps, &info.sender.to_string())?;
    let config = Config::load(deps.storage)?;
    if ido.remaining_tokens_per_tier.len() != config.min_tier as usize {
        return Err(ContractError::Std(StdError::generic_err(
            "`tokens_per_tier` has wrong size",
        )));
    }

    let sum = ido.remaining_tokens_per_tier.iter().sum::<u128>();
    if sum < ido.total_tokens_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "Sum of `tokens_per_tier` can't be less than total tokens amount",
        )));
    }

    if ido.start_time >= ido.end_time {
        return Err(ContractError::Std(StdError::generic_err(
            "End time must be greater than start time",
        )));
    }

    if ido.price == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "Ido price should be initialized",
        )));
    }
    if env.block.time.seconds() >= ido.end_time {
        return Err(ContractError::Std(StdError::generic_err(
            "Ido ends in the past",
        )));
    }

    if ido.soft_cap == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "soft_cap should be initialized.",
        )));
    }

    if ido.soft_cap > ido.total_tokens_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "soft_cap should be less than total amount",
        )));
    }
    ido.shared_whitelist = match whitelist {
        Whitelist::Shared { .. } => true,
        Whitelist::Empty { .. } => false,
    };

    let ido_id = ido.save(deps.storage)?;
    // let ido_whitelist = state::ido_whitelist(ido_id);

    match whitelist {
        Whitelist::Empty { with } => {
            for address in with.unwrap_or_default() {
                let canonical_address = address.to_string();
                WHITELIST.save(deps.storage, (ido_id, canonical_address), &true)?;
            }
        }
        Whitelist::Shared { with_blocked } => {
            for address in with_blocked.unwrap_or_default() {
                let canonical_address = address.to_string();
                WHITELIST.save(deps.storage, (ido_id, canonical_address), &false)?;
            }
        }
    }

    ido.save(deps.storage)?;

    let canonical_sender = info.sender.to_string();

    let mut startup_ido_list = OWNER_TO_IDOS
        .may_load(deps.storage, canonical_sender)?
        .unwrap_or_default();
    startup_ido_list.push(ido_id);
    OWNER_TO_IDOS.save(deps.storage, info.sender.to_string(), &startup_ido_list)?;

    let token_address = ido.token_contract.to_string();
    let transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.to_string(),
        recipient: env.contract.address.to_string(),
        amount: Uint128::new(ido.total_tokens_amount),
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_address,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    });

    let answer = to_binary(&ExecuteResponse::StartIdo {
        ido_id,
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().set_data(answer).add_submessage(sub_msg))
}

fn buy_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    mut amount: u128,
    viewing_key: Option<String>,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;

    let sender = info.sender.to_string();
    let canonical_sender = sender.to_string();

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if !ido.is_active(env.block.time.seconds()) {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "IDO is not active {}",
            env.block.time,
        ))));
    }

    if ido.is_native_payment() {
        let orai_amount = utils::sent_funds(&info.funds)?;
        amount = orai_amount.checked_mul(ido.price).unwrap();
    }

    if amount == 0 {
        return Err(ContractError::Std(StdError::generic_err("Zero amount")));
    }

    let config = Config::load(deps.storage)?;
    let tier = if utils::in_whitelist(deps.storage, &sender, ido_id)? {
        get_tier(&deps, sender.clone(), viewing_key.clone())?
    } else {
        config.min_tier
    };

    let remaining_amount = ido.remaining_tokens_per_tier(tier);
    if remaining_amount == 0 {
        if ido.total_tokens_amount == ido.sold_amount {
            return Err(ContractError::Std(StdError::generic_err(
                "All tokens are sold",
            )));
        } else {
            return Err(ContractError::Std(StdError::generic_err(
                "All tokens are sold for your tier",
            )));
        }
    }

    if amount > remaining_amount {
        let msg = format!("You cannot buy more than {} tokens", remaining_amount);
        return Err(ContractError::Std(StdError::generic_err(&msg)));
    }

    let payment = amount.checked_div(ido.price).unwrap();
    let lock_period = config.lock_period(tier);

    let unlock_time = ido.end_time.checked_add(lock_period).unwrap();
    let tokens_amount = Uint128::new(amount);
    let purchase = Purchase {
        timestamp: env.block.time.seconds(),
        tokens_amount: tokens_amount.u128(),
        unlock_time,
    };

    let mut purchases = PURCHASES
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();
    purchases.push(purchase);
    PURCHASES.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &purchases,
    )?;

    let mut user_ido_info = IDO_TO_INFO
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();

    if user_ido_info.total_payment == 0 {
        ido.participants = ido.participants.checked_add(1).unwrap();
    }

    user_ido_info.total_payment = user_ido_info.total_payment.checked_add(payment).unwrap();
    user_ido_info.total_tokens_bought = user_ido_info
        .total_tokens_bought
        .checked_add(amount)
        .unwrap();

    let mut user_info = USERINFO
        .may_load(deps.storage, canonical_sender.to_string())?
        .unwrap_or_default();

    user_info.total_payment = user_info.total_payment.checked_add(payment).unwrap();
    user_info.total_tokens_bought = user_info.total_tokens_bought.checked_add(amount).unwrap();

    USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

    IDO_TO_INFO.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &user_ido_info,
    )?;

    ACTIVE_IDOS.save(deps.storage, (canonical_sender.to_string(), ido_id), &true)?;

    ido.sold_amount = ido.sold_amount.checked_add(amount).unwrap();
    ido.total_payment = ido.total_payment.checked_add(payment).unwrap();

    let tier_index = tier.checked_sub(1).unwrap() as usize;
    ido.remaining_tokens_per_tier[tier_index] = ido.remaining_tokens_per_tier[tier_index]
        .checked_sub(amount)
        .unwrap();

    ido.save(deps.storage)?;

    let answer = to_binary(&ExecuteResponse::BuyTokens {
        unlock_time,
        amount: Uint128::new(amount),
        status: ResponseStatus::Success,
    })?;

    if !ido.is_native_payment() {
        let token_contract_canonical = ido.payment_token_contract.unwrap();
        // let token_contract_hash = ido.payment_token_hash.unwrap();
        let token_contract = token_contract_canonical.to_string();

        let transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: info.sender.to_string(),
            recipient: env.contract.address.to_string(),
            amount: Uint128::new(payment),
        };

        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: token_contract,
            msg: to_binary(&transfer_msg)?,
            funds: vec![],
        });

        return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
    }
    // else ---> scrt tokens are in the contract itself.
    Ok(Response::new().set_data(answer))
}

fn recv_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
    start: Option<u32>,
    limit: Option<u32>,
    purchase_indices: Option<Vec<u32>>,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    //
    let canonical_sender = info.sender.to_string();
    let current_time = env.block.time;

    let ido = Ido::load(deps.storage, ido_id)?;
    let mut user_info = USERINFO
        .may_load(deps.storage, canonical_sender.to_string())?
        .unwrap_or_default();
    let mut user_ido_info = IDO_TO_INFO
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();

    // when ido failed, withdraw the payment tokens.
    if current_time.seconds() > ido.end_time && ido.soft_cap > ido.sold_amount {
        user_info.total_payment = user_info
            .total_payment
            .checked_sub(user_ido_info.total_payment)
            .unwrap_or_default();
        user_info.total_tokens_bought = user_info
            .total_payment
            .checked_sub(user_ido_info.total_tokens_bought)
            .unwrap_or_default();
        user_ido_info.total_tokens_received = 0;
        user_ido_info.total_tokens_bought = 0;
        user_ido_info.total_payment = 0;

        USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

        IDO_TO_INFO.save(
            deps.storage,
            (canonical_sender.to_string(), ido_id),
            &user_ido_info,
        )?;
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));

        let answer = to_binary(&ExecuteResponse::RecvTokens {
            amount: Uint128::new(user_info.total_payment),
            status: ResponseStatus::Success,
            ido_success: false,
        })?;

        if ido.is_native_payment() {
            let transfer_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(user_ido_info.total_payment, ORAI),
            });
            return Ok(Response::new().set_data(answer).add_message(transfer_msg));
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();

            let transfer_msg = Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.to_string(),
                recipient: env.contract.address.to_string(),
                amount: Uint128::new(user_ido_info.total_payment),
            };

            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_binary(&transfer_msg)?,
                funds: vec![],
            });
            return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
        };
    }
    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(300);
    let mut purchases = PURCHASES
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();
    let purchases_iter = purchases.iter().skip(start as usize).take(limit as usize);

    let mut indices = Vec::new();
    for (i, purchase) in purchases_iter.enumerate() {
        if current_time.seconds() >= purchase.unlock_time {
            let index = i.checked_add(start as usize).unwrap();
            indices.push(index);
        }
    }

    if let Some(purchase_indices) = purchase_indices {
        let end = start.checked_add(limit).unwrap();
        for index in purchase_indices {
            if index >= start && index < end {
                continue;
            }

            let purchase = purchases.get(index as usize).unwrap();
            if current_time.seconds() >= purchase.unlock_time {
                indices.push(index as usize);
            }
        }
    }

    indices.sort();
    indices.dedup();

    let mut recv_amount: u128 = 0;

    let mut archived_purchases = ARCHIVED_PURCHASES
        .may_load(deps.storage, (canonical_sender.to_string(), ido_id))?
        .unwrap_or_default();

    for (shift, index) in indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        let purchase = purchases.remove(position as usize);

        recv_amount = recv_amount.checked_add(purchase.tokens_amount).unwrap();
        archived_purchases.push(purchase);
    }
    PURCHASES.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &purchases,
    )?;
    ARCHIVED_PURCHASES.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &archived_purchases,
    )?;

    if recv_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to receive",
        )));
    }

    let answer = to_binary(&ExecuteResponse::RecvTokens {
        amount: Uint128::new(recv_amount),
        status: ResponseStatus::Success,
        ido_success: true,
    })?;

    user_info.total_tokens_received = user_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    user_ido_info.total_tokens_received = user_ido_info
        .total_tokens_received
        .checked_add(recv_amount)
        .unwrap();

    USERINFO.save(deps.storage, canonical_sender.to_string(), &user_info)?;

    IDO_TO_INFO.save(
        deps.storage,
        (canonical_sender.to_string(), ido_id),
        &user_ido_info,
    )?;

    if user_ido_info.total_tokens_bought == user_ido_info.total_tokens_received {
        ACTIVE_IDOS.remove(deps.storage, (canonical_sender.to_string(), ido_id));
    }

    let token_contract = ido.token_contract.to_string();

    let transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.to_string(),
        recipient: env.contract.address.to_string(),
        amount: Uint128::new(recv_amount),
    };

    let sub_msg = SubMsg::new(WasmMsg::Execute {
        contract_addr: token_contract,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    });
    return Ok(Response::new().set_data(answer).add_submessage(sub_msg));
}

fn withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    ido_id: u32,
) -> Result<Response, ContractError> {
    let ido_admin = info.sender.to_string();
    assert_ido_admin(&deps, &ido_admin, ido_id)?;
    assert_contract_active(deps.storage)?;

    let mut ido = Ido::load(deps.storage, ido_id)?;
    if ido.withdrawn {
        return Err(ContractError::Std(StdError::generic_err(
            "Already withdrawn",
        )));
    }

    if env.block.time.seconds() < ido.end_time {
        return Err(ContractError::Std(StdError::generic_err(
            "IDO is not finished yet",
        )));
    }

    ido.withdrawn = true;
    ido.save(deps.storage)?;

    let remaining_tokens: Uint128;
    if ido.soft_cap > ido.sold_amount {
        remaining_tokens = Uint128::from(ido.total_tokens_amount);
    } else {
        remaining_tokens = Uint128::from(ido.remaining_tokens());
    }

    let ido_token_contract = ido.token_contract.to_string();

    let mut msgs = vec![];
    let mut submsgs = vec![];
    if !remaining_tokens.is_zero() {
        let transfer_msg = Cw20ExecuteMsg::TransferFrom {
            owner: ido_admin.to_string(),
            recipient: env.contract.address.to_string(),
            amount: remaining_tokens,
        };

        let sub_msg = SubMsg::new(WasmMsg::Execute {
            contract_addr: ido_token_contract,
            msg: to_binary(&transfer_msg)?,
            funds: vec![],
        });

        submsgs.push(sub_msg);
    }
    //withdraw payment tokens.
    let payment_amount = Uint128::new(ido.sold_amount.checked_div(ido.price).unwrap());
    if ido.sold_amount >= ido.soft_cap {
        if ido.is_native_payment() {
            msgs.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: ido_admin,
                amount: coins(ido.sold_amount.checked_div(ido.price).unwrap(), ORAI),
            }))
        } else {
            let token_contract_canonical = ido.payment_token_contract.unwrap();
            // let token_contract_hash = ido.payment_token_hash.unwrap();
            let token_contract = token_contract_canonical.to_string();

            let transfer_msg = Cw20ExecuteMsg::TransferFrom {
                owner: ido_admin.to_string(),
                recipient: env.contract.address.to_string(),
                amount: payment_amount,
            };

            let sub_msg = SubMsg::new(WasmMsg::Execute {
                contract_addr: token_contract,
                msg: to_binary(&transfer_msg)?,
                funds: vec![],
            });

            submsgs.push(sub_msg);
        };
    }

    let answer = to_binary(&ExecuteResponse::Withdraw {
        ido_amount: remaining_tokens,
        payment_amount: payment_amount,
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new()
        .set_data(answer)
        .add_messages(msgs)
        .add_submessages(submsgs));
}

fn whitelist_add(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
    ido_id: u32,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_ido_admin(&deps, &info.sender.to_string(), ido_id)?;

    // let whitelist = state::ido_whitelist(ido_id);
    for address in addresses {
        let canonical_address = address.to_string();
        WHITELIST.save(deps.storage, (ido_id, canonical_address), &true)?;
    }

    let answer = to_binary(&ExecuteResponse::WhitelistAdd {
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().set_data(answer));
}

fn whitelist_remove(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    addresses: Vec<String>,
    ido_id: u32,
) -> Result<Response, ContractError> {
    assert_contract_active(deps.storage)?;
    assert_ido_admin(&deps, &info.sender.to_string(), ido_id)?;

    // let whitelist = state::ido_whitelist(ido_id);

    for address in addresses {
        let canonical_address = address.to_string();
        WHITELIST.save(deps.storage, (ido_id, canonical_address), &false)?;
    }

    let answer = to_binary(&ExecuteResponse::WhitelistRemove {
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().set_data(answer));
}

fn try_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG_KEY.load(deps.storage)?;
    config.assert_contract_active()?;

    let received_funds = get_received_funds(&deps, &info)?;

    let mut orai_deposit = received_funds.amount.u128();

    let band_protocol = BandProtocol::new(&deps)?;

    let usd_deposit = band_protocol.usd_amount(orai_deposit);

    let sender = info.sender.to_string();
    let min_tier = config.min_tier();

    let mut user_info =
        TIER_USER_INFOS
            .may_load(deps.storage, sender)?
            .unwrap_or(state::TierUserInfo {
                tier: min_tier,
                ..Default::default()
            });
    let current_tier = user_info.tier;
    let old_usd_deposit = user_info.usd_deposit;
    let new_usd_deposit = old_usd_deposit.checked_add(usd_deposit).unwrap();

    let new_tier = config.tier_by_deposit(new_usd_deposit);

    if current_tier == new_tier {
        if current_tier == config.max_tier() {
            return Err(ContractError::Std(StdError::generic_err(
                "Reached max tier",
            )));
        }

        let next_tier = current_tier.checked_sub(1).unwrap();
        let next_tier_deposit = config.deposit_by_tier(next_tier);

        let expected_deposit_usd = next_tier_deposit.checked_sub(old_usd_deposit).unwrap();
        let expected_deposit_scrt = band_protocol.orai_amount(expected_deposit_usd);

        let err_msg = format!(
            "You should deposit at least {} USD ({} ORAI)",
            expected_deposit_usd, expected_deposit_scrt
        );

        return Err(ContractError::Std(StdError::generic_err(&err_msg)));
    }

    let mut messages: Vec<SubMsg> = Vec::with_capacity(2);
    let new_tier_deposit = config.deposit_by_tier(new_tier);

    let usd_refund = new_usd_deposit.checked_sub(new_tier_deposit).unwrap();
    let orai_refund = band_protocol.orai_amount(usd_refund);

    if orai_refund != 0 {
        orai_deposit = orai_deposit.checked_sub(orai_refund).unwrap();

        let send_msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(orai_refund, ORAI),
        };

        let msg = CosmosMsg::Bank(send_msg);
        messages.push(SubMsg::new(msg));
    }
    let old_orai_deposit = user_info.orai_deposit;
    user_info.tier = new_tier;
    user_info.timestamp = env.block.time.seconds();
    user_info.usd_deposit = new_tier_deposit;
    user_info.orai_deposit = user_info.orai_deposit.checked_add(orai_deposit).unwrap();
    TIER_USER_INFOS.save(deps.storage, info.sender.to_string(), &user_info)?;

    let delegate_msg = StakingMsg::Delegate {
        validator: config.validator,
        amount: coin(
            user_info
                .orai_deposit
                .checked_sub(old_orai_deposit)
                .unwrap(),
            ORAI,
        ),
    };

    let msg = CosmosMsg::Staking(delegate_msg);
    messages.push(SubMsg::new(msg));

    let answer = to_binary(&ExecuteResponse::Deposit {
        usd_deposit: Uint128::new(user_info.usd_deposit),
        orai_deposit: Uint128::new(user_info.orai_deposit),
        tier: new_tier,
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_submessages(messages).set_data(answer))
}

pub fn withdraw_from_tier(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG_KEY.load(deps.storage)?;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();

    let min_tier = config.min_tier();
    let user_info =
        TIER_USER_INFOS
            .may_load(deps.storage, sender)?
            .unwrap_or(state::TierUserInfo {
                tier: min_tier,
                ..Default::default()
            });

    let amount = user_info.orai_deposit;

    TIER_USER_INFOS.remove(deps.storage, info.sender.to_string());

    let current_time = env.block.time.seconds();
    let claim_time = current_time.checked_add(UNBOUND_LATENCY).unwrap();
    let withdrawal = UserWithdrawal {
        amount,
        timestamp: current_time,
        claim_time,
    };

    let mut withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, info.sender.to_string())?
        .unwrap_or_default();

    withdrawals.push(withdrawal);
    WITHDRAWALS_LIST.save(deps.storage, info.sender.to_string(), &withdrawals)?;

    let validator = config.validator;
    let amount = coin(amount - 4, ORAI);

    let withdraw_msg = StakingMsg::Undelegate { validator, amount };
    let msg = CosmosMsg::Staking(withdraw_msg);

    let answer = to_binary(&ExecuteResponse::WithdrawFromTier {
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Option<String>,
    start: Option<u32>,
    limit: Option<u32>,
) -> Result<Response, ContractError> {
    let config = CONFIG_KEY.load(deps.storage)?;
    config.assert_contract_active()?;

    let sender = info.sender.to_string();
    let mut withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, sender)?
        .unwrap_or_default();

    let length = withdrawals.len();

    if length == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to claim",
        )));
    }

    let recipient = recipient.unwrap_or(info.sender.to_string());
    let start: usize = start.unwrap_or(0) as usize;
    let limit = limit.unwrap_or(50) as usize;
    let withdrawals_iter: std::iter::Take<std::iter::Skip<std::slice::Iter<'_, UserWithdrawal>>> =
        withdrawals.iter().skip(start).take(limit);

    let current_time = env.block.time.seconds();
    let mut remove_indices = Vec::new();
    let mut claim_amount = 0u128;

    for (index, withdrawal) in withdrawals_iter.enumerate() {
        let claim_time = withdrawal.claim_time;

        if current_time >= claim_time {
            remove_indices.push(index);
            claim_amount = claim_amount.checked_add(withdrawal.amount).unwrap();
        }
    }

    if claim_amount == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "Nothing to claim",
        )));
    }

    for (shift, index) in remove_indices.into_iter().enumerate() {
        let position = index.checked_sub(shift).unwrap();
        withdrawals.remove(position);
    }

    let send_msg = BankMsg::Send {
        to_address: recipient,
        amount: coins(claim_amount, ORAI),
    };

    let msg = CosmosMsg::Bank(send_msg);
    let answer = to_binary(&ExecuteResponse::Claim {
        amount: claim_amount.into(),
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_withdraw_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    _recipient: Option<String>,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG_KEY.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    let validator = config.validator;
    let delegation = utils::query_delegation(&deps, &env, &validator);

    let can_withdraw = delegation
        .map(|d| d.unwrap().accumulated_rewards[0].amount.u128())
        .unwrap_or(0);

    if can_withdraw == 0 {
        return Err(ContractError::Std(StdError::generic_err(
            "There is nothing to withdraw",
        )));
    }

    // let admin = config.admin;
    // let recipient = recipient.unwrap_or(admin);
    let withdraw_msg = DistributionMsg::WithdrawDelegatorReward { validator };

    let msg = CosmosMsg::Distribution(withdraw_msg);
    let answer = to_binary(&ExecuteResponse::WithdrawRewards {
        amount: Uint128::new(can_withdraw),
        status: ResponseStatus::Success,
    })?;

    Ok(Response::new().add_message(msg).set_data(answer))
}

pub fn try_redelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    validator_address: String,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG_KEY.load(deps.storage)?;
    if info.sender.clone() != config.admin {
        return Err(ContractError::Std(StdError::generic_err("Unauthorized")));
    }

    let old_validator = config.validator;
    let delegation = utils::query_delegation(&deps, &env, &old_validator);

    if old_validator == validator_address {
        return Err(ContractError::Std(StdError::generic_err(
            "Redelegation to the same validator",
        )));
    }

    if delegation.is_err() {
        config.validator = validator_address;
        CONFIG_KEY.save(deps.storage, &config)?;

        let answer = to_binary(&ExecuteResponse::Redelegate {
            amount: Uint128::zero(),
            status: ResponseStatus::Success,
        })?;

        return Ok(Response::new().set_data(answer));
    }

    let delegation = delegation.unwrap().unwrap();
    let can_withdraw = delegation.accumulated_rewards[0].amount.u128();
    let can_redelegate = delegation.can_redelegate.amount.u128();
    let delegated_amount = delegation.amount.amount.u128();

    if can_redelegate != delegated_amount {
        return Err(ContractError::Std(StdError::generic_err(
            "Cannot redelegate full delegation amount",
        )));
    }

    config.validator = validator_address.clone();
    CONFIG_KEY.save(deps.storage, &config)?;

    let mut messages = Vec::with_capacity(2);
    if can_withdraw != 0 {
        let admin = config.admin;
        let _recipient = recipient.unwrap_or(admin);
        let withdraw_msg = DistributionMsg::WithdrawDelegatorReward {
            validator: old_validator.clone(),
        };

        let msg = CosmosMsg::Distribution(withdraw_msg);

        messages.push(msg);
    }

    let coin = coin(can_redelegate, ORAI);
    let redelegate_msg = StakingMsg::Redelegate {
        src_validator: old_validator,
        dst_validator: validator_address,
        amount: coin,
    };

    messages.push(CosmosMsg::Staking(redelegate_msg));
    let answer = to_binary(&ExecuteResponse::Redelegate {
        amount: Uint128::new(can_redelegate),
        status: ResponseStatus::Success,
    })?;

    return Ok(Response::new().add_messages(messages).set_data(answer));
}

pub fn query_config(deps: Deps) -> StdResult<QueryResponse> {
    let config = CONFIG_KEY.load(deps.storage)?;
    config.to_answer()
}

pub fn query_user_info(deps: Deps, address: String) -> StdResult<QueryResponse> {
    let config = CONFIG_KEY.load(deps.storage)?;
    let min_tier = config.min_tier();
    let user_info =
        TIER_USER_INFOS
            .may_load(deps.storage, address)?
            .unwrap_or(state::TierUserInfo {
                tier: min_tier,
                ..Default::default()
            });

    let answer = user_info.to_answer();
    return Ok(answer);
}

pub fn query_withdrawals(
    deps: Deps,
    address: String,
    start: Option<u32>,
    limit: Option<u32>,
) -> StdResult<QueryResponse> {
    let withdrawals = WITHDRAWALS_LIST
        .may_load(deps.storage, address)?
        .unwrap_or_default();
    let amount = withdrawals.len();

    let start = start.unwrap_or(0);
    let limit = limit.unwrap_or(50);

    // let withdrawals = withdrawals.partition_point(pred) .paging(&deps.storage, start, limit)?;
    // let serialized_withdrawals = withdrawals.into_iter().map(|w| w.to_serialized()).collect();

    let mut serialized_withdrawals: Vec<SerializedWithdrawals> = Vec::new();
    for i in start..start + limit {
        let index: usize = i.try_into().unwrap();
        if index < amount {
            serialized_withdrawals.push(withdrawals[index].to_serialized())
        }
    }

    let answer = QueryResponse::Withdrawals {
        amount: amount.try_into().unwrap(),
        withdrawals: serialized_withdrawals,
    };

    Ok(answer)
}

// #[cfg(test)]
// mod tests {
//     use std::marker::PhantomData;
//     use std::time::{SystemTime, UNIX_EPOCH};

//     use crate::tier::manual;

//     use super::*;
//     use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockQuerier, MockStorage};
//     use cosmwasm_std::{from_binary, OwnedDeps};
//     use rand::{thread_rng, Rng};

//     fn get_init_msg() -> InstantiateMsg {
//         InstantiateMsg {
//             admin: None,
//             tier_contract: "tier".to_string(),
//             nft_contract: "nft".to_string(),
//             lock_periods: vec![250, 200, 150, 100],
//         }
//     }

//     fn initialize_with(
//         msg: InstantiateMsg,
//     ) -> OwnedDeps<cosmwasm_std::MemoryStorage, MockApi, MockQuerier, PhantomData> {
//         let mut deps = OwnedDeps {
//             storage: MockStorage::default(),
//             api: MockApi::default(),
//             querier: MockQuerier::default(),
//             custom_query_type: PhantomData::default(),
//         };
//         let info: MessageInfo = mock_info("admin", &coins(2, "orai"));

//         instantiate(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();

//         deps
//     }

//     fn initialize_with_default(
//     ) -> OwnedDeps<cosmwasm_std::MemoryStorage, MockApi, MockQuerier, PhantomData> {
//         let msg = get_init_msg();
//         initialize_with(msg).unwrap()
//     }

//     fn start_ido_msg() -> ExecuteMsg {
//         let mut rng = thread_rng();
//         let token_contract = format!("token_{}", rng.gen_range(0..1000));

//         let start_time = SystemTime::now()
//             .duration_since(UNIX_EPOCH)
//             .unwrap()
//             .as_secs();

//         let end_time = start_time + rng.gen::<u64>();

//         let price = rng.gen();
//         let total_amount = rng.gen();

//         let mut whitelist = Vec::new();
//         for i in 0..rng.gen_range(20..100) {
//             let address = format!("address_{}", i);
//             whitelist.push(address);
//         }

//         let mut tokens_per_tier = Vec::new();
//         let mut remaining_tokens = total_amount;
//         for _ in 0..3 {
//             let tokens_amount = rng.gen_range(0..=remaining_tokens);
//             tokens_per_tier.push(Uint128::new(tokens_amount));
//             remaining_tokens -= tokens_amount;
//         }
//         tokens_per_tier.push(Uint128::new(remaining_tokens));

//         ExecuteMsg::StartIdo {
//             start_time,
//             end_time,
//             token_contract: token_contract,
//             payment: PaymentMethod::Token {
//                 contract: "token".to_string(),
//                 code_hash: String::from("token_hash"),
//             },
//             price: Uint128::new(price),
//             total_amount: Uint128::new(total_amount),
//             soft_cap: Uint128::new(total_amount),
//             whitelist: Whitelist::Empty {
//                 with: Some(whitelist),
//             },
//             tokens_per_tier,
//             padding: None,
//         }
//     }

//     fn extract_error(response: Result<Response, ContractError>) -> String {
//         match response {
//             Ok(_) => panic!("Response is not an error"),
//             Err(err) => match err {
//                 ContractError::Std(StdError::GenericErr { msg, .. }) => msg,
//                 ContractError::Unauthorized { .. } => "Unauthorized".into(),
//                 _ => panic!("Unexpected error"),
//             },
//         }
//     }

//     #[test]
//     fn initialize() {
//         let msg = get_init_msg();
//         let mut deps = initialize_with(msg.clone()).unwrap();

//         let config: Config = Config::load(&deps.storage).unwrap();

//         let min_tier = manual::get_min_tier(&deps.as_mut(), &config).unwrap();

//         let admin = "admin".to_string();

//         assert_eq!(config.admin, admin);
//         assert_eq!(config.lock_periods, msg.lock_periods);
//         assert_eq!(config.tier_contract, msg.tier_contract.to_string());
//         assert_eq!(config.nft_contract, msg.nft_contract.to_string());
//         assert_eq!(config.min_tier, min_tier);
//     }

//     #[test]
//     fn initialize_with_wrong_lock_periods() {
//         let mut msg = get_init_msg();
//         msg.lock_periods = vec![1, 2, 3];

//         let mut deps = OwnedDeps {
//             storage: MockStorage::default(),
//             api: MockApi::default(),
//             querier: MockQuerier::default(),
//             custom_query_type: PhantomData::default(),
//         };
//         let info: MessageInfo = mock_info("admin", &coins(2, "orai"));

//         let response = instantiate(deps.as_mut(), mock_env(), info, msg.clone());
//         let error = extract_error(response);

//         assert!(error.contains("Lock periods array must have 4 items"));
//     }

//     #[test]
//     fn start_ido() {
//         let mut deps = initialize_with_default();

//         let ido_admin = "admin".to_string();
//         let canonical_ido_admin = ido_admin.to_string();
//         let info: MessageInfo = mock_info(&ido_admin, &[]);
//         let env = mock_env();
//         let msg = start_ido_msg();

//         let startup_ido_list = OWNER_TO_IDOS
//             .may_load(&deps.storage, canonical_ido_admin.clone())
//             .unwrap_or_default()
//             .unwrap_or_default();
//         assert_eq!(startup_ido_list.len(), 0);
//         assert_eq!(Ido::len(&deps.storage), Ok(0));

//         let response = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
//         let messages = response.messages;
//         let data = response.data;

//         match from_binary(&data.unwrap()).unwrap() {
//             ExecuteResponse::StartIdo { ido_id, status, .. } => {
//                 assert_eq!(ido_id, 0);
//                 assert_eq!(status, ResponseStatus::Success);
//             }
//             _ => unreachable!(),
//         }

//         assert_eq!(Ido::len(&deps.storage), Ok(1));
//         let ido = Ido::load(&deps.storage, 0).unwrap();

//         let startup_ido_list = OWNER_TO_IDOS
//             .may_load(&deps.storage, canonical_ido_admin)
//             .unwrap_or_default()
//             .unwrap_or_default();

//         assert_eq!(startup_ido_list.len(), 1);

//         if let ExecuteMsg::StartIdo {
//             start_time,
//             end_time,
//             token_contract,
//             price,
//             total_amount,
//             whitelist: _whitelist,
//             payment,
//             ..
//         } = msg
//         {
//             let sender = info.sender.to_string();
//             let token_contract_canonical = token_contract.to_string();

//             let payment_token_contract_canonical = match payment {
//                 PaymentMethod::Native => None,
//                 PaymentMethod::Token { contract, .. } => Some(contract),
//             };

//             assert_eq!(ido.admin, sender);
//             assert_eq!(ido.start_time, start_time);
//             assert_eq!(ido.end_time, end_time);
//             assert_eq!(ido.token_contract, token_contract_canonical);
//             assert_eq!(ido.price, price.u128());
//             assert_eq!(ido.participants, 0);
//             assert_eq!(ido.sold_amount, 0);
//             assert_eq!(ido.total_tokens_amount, total_amount.u128());
//             assert_eq!(ido.payment_token_contract, payment_token_contract_canonical);
//             assert_eq!(ido.payment_token_hash, Some(String::from("token_hash")));

//             let transfer_msg = Cw20ExecuteMsg::TransferFrom {
//                 owner: info.sender.to_string(),
//                 recipient: env.contract.address.to_string(),
//                 amount: total_amount,
//             };

//             let sub_msg = SubMsg::new(WasmMsg::Execute {
//                 contract_addr: token_contract,
//                 msg: to_binary(&transfer_msg).unwrap(),
//                 funds: vec![],
//             });
//             assert_eq!(messages.len(), 1);
//             assert_eq!(messages[0], sub_msg);
//         } else {
//             unreachable!();
//         }
//     }
// }
