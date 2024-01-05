use cosmwasm_std::{Decimal, StdResult};

use cosmwasm_std::DepsMut;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Config;

pub struct BandProtocol {
    orai_per_usd: u128,
}

impl BandProtocol {
    pub const DECIMALS: u8 = 18;
    pub const ONE_USD: u128 = 1_000_000_000_000_000_000;

    pub fn new(deps: &DepsMut) -> StdResult<Self> {
        // let querier: SeiQuerier<'_> = SeiQuerier::new(&deps.querier);
        // let res = querier
        //     .query_exchange_rates()
        //     .unwrap_or(ExchangeRatesResponse {
        //         denom_oracle_exchange_rate_pairs: vec![],
        //     });

        // let mut orai_per_usd = Self::ONE_USD / 2;
        // for exratepair in res.denom_oracle_exchange_rate_pairs {
        //     if exratepair.denom.clone() == "usei" {
        //         let rate = exratepair.oracle_exchange_rate.exchange_rate;
        //         orai_per_usd = (Decimal::raw(1000000u128) / rate).to_uint_floor().u128();
        //     }
        // }
        let config = Config::load(deps.storage)?;
        let orai_contract = config.oraiswap_contract.orai_contract;
        let native_token = NativeToken::new("orai".to_string());
        let offer_asset_info = OfferAssetInfo::new(native_token);
        let usdt_contract_address = config.oraiswap_contract.usdt_contract;
        let msg = SwapContractMessage {
            simulate_swap_operations: SwapCtrMessageContent {
                offer_amount: 1000000,
                operations: vec![Operation {
                    orai_swap: OraiSwap {
                        offer_asset_info: offer_asset_info,
                        ask_asset_info: AskAssetInfo {
                            token: UsdtContractAddr {
                                contract_addr: usdt_contract_address,
                            },
                        },
                    },
                }],
            },
        };
        let response: ChangeRateResponse = deps.querier.query_wasm_smart(orai_contract, &msg)?;
        let rate = response.data.amount;
        // let rate = 8123456;
        let orai_per_usd = (Decimal::raw(1000000u128) / Decimal::raw(rate))
            .to_uint_floor()
            .u128();
        Ok(BandProtocol { orai_per_usd })
    }

    pub fn usd_amount(&self, usei: u128) -> u128 {
        usei.checked_mul(self.orai_per_usd)
            .and_then(|v| v.checked_div(BandProtocol::ONE_USD))
            .unwrap()
    }

    pub fn orai_amount(&self, usd: u128) -> u128 {
        usd.checked_mul(BandProtocol::ONE_USD)
            .and_then(|v: u128| v.checked_div(self.orai_per_usd))
            .unwrap()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct for the innermost part "native_token"
pub struct NativeToken {
    denom: String,
}

impl NativeToken {
    pub fn new(native_token_denom: String) -> Self {
        NativeToken {
            denom: native_token_denom,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct associated with "offer_asset_info"
pub struct OfferAssetInfo {
    native_token: NativeToken,
}

impl OfferAssetInfo {
    pub fn new(native_token: NativeToken) -> Self {
        OfferAssetInfo {
            native_token: native_token,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UsdtContractAddr {
    contract_addr: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AskAssetInfo {
    pub token: UsdtContractAddr,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
// Define the struct represented by the "orai_swap" key
pub struct OraiSwap {
    offer_asset_info: OfferAssetInfo,
    ask_asset_info: AskAssetInfo,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Operation {
    pub orai_swap: OraiSwap,
}

impl Clone for Operation {
    fn clone(&self) -> Operation {
        Operation {
            orai_swap: self.orai_swap.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SwapCtrMessageContent {
    pub offer_amount: u128,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SwapContractMessage {
    pub simulate_swap_operations: SwapCtrMessageContent,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Amount {
    amount: u128
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ChangeRateResponse {
    pub data: Amount   
}