use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    Active,
    Stopped,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct NftToken {
    pub token_id: String,
    pub viewing_key: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ValidatorWithWeight {
    pub address: String,
    pub weight: u128,
}

impl Clone for ValidatorWithWeight {
    fn clone(&self) -> ValidatorWithWeight {
        ValidatorWithWeight {
            address: self.address.clone(),
            weight: self.weight.clone(), // Handle other fields accordingly.
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct OraiswapContract {
    pub orai_contract: String,
    pub usdt_contract: String,
}

impl Clone for OraiswapContract {
    fn clone(&self) -> OraiswapContract {
        OraiswapContract {
            orai_contract: self.orai_contract.clone(),
            usdt_contract: self.usdt_contract.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub admin: Option<String>,
    pub lock_periods: Vec<u64>,
    pub nft_contract: String,
    pub validators: Vec<ValidatorWithWeight>, // Tier Contract
    pub deposits: Vec<Uint128>,               // Tier Contract
    pub oraiswap_contract: OraiswapContract,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Native,
    Token { contract: String, code_hash: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Whitelist {
    Empty { with: Option<Vec<String>> },
    Shared { with_blocked: Option<Vec<String>> },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ChangeAdmin {
        admin: String,
        padding: Option<String>,
    },
    ChangeStatus {
        status: ContractStatus,
        padding: Option<String>,
    },
    StartIdo {
        start_time: u64,
        end_time: u64,
        token_contract: String,
        price: Uint128,
        soft_cap: Uint128,
        payment: PaymentMethod,
        total_amount: Uint128,
        tokens_per_tier: Vec<Uint128>,
        padding: Option<String>,
        whitelist: Whitelist,
    },
    WhitelistAdd {
        addresses: Vec<String>,
        ido_id: u32,
        padding: Option<String>,
    },
    WhitelistRemove {
        addresses: Vec<String>,
        ido_id: u32,
        padding: Option<String>,
    },
    BuyTokens {
        ido_id: u32,
        amount: Uint128,
        viewing_key: Option<String>,
        padding: Option<String>,
    },
    RecvTokens {
        ido_id: u32,
        start: Option<u32>,
        limit: Option<u32>,
        purchase_indices: Option<Vec<u32>>,
        padding: Option<String>,
    },
    Withdraw {
        ido_id: u32,
        padding: Option<String>,
    },
    // Tier
    Deposit {
        padding: Option<String>,
    },
    WithdrawFromTier {
        padding: Option<String>,
    },
    Claim {
        recipient: Option<String>,
        start: Option<u32>,
        limit: Option<u32>,
        padding: Option<String>,
    },
    WithdrawRewards {
        recipient: Option<String>,
        padding: Option<String>,
    },
    Redelegate {
        validator_address: String,
        recipient: Option<String>,
        padding: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteResponse {
    ChangeAdmin {
        status: ResponseStatus,
    },
    ChangeStatus {
        status: ResponseStatus,
    },
    StartIdo {
        ido_id: u32,
        status: ResponseStatus,
    },
    WhitelistAdd {
        status: ResponseStatus,
    },
    WhitelistRemove {
        status: ResponseStatus,
    },
    BuyTokens {
        amount: Uint128,
        unlock_time: u64,
        status: ResponseStatus,
    },
    RecvTokens {
        amount: Uint128,
        status: ResponseStatus,
        ido_success: bool,
    },
    Withdraw {
        ido_amount: Uint128,
        payment_amount: Uint128,
        status: ResponseStatus,
    },
    // Tier Contrac
    Deposit {
        usd_deposit: Uint128,
        orai_deposit: Uint128,
        tier: u8,
        status: ResponseStatus,
    },
    WithdrawFromTier {
        status: ResponseStatus,
    },
    Claim {
        amount: Uint128,
        status: ResponseStatus,
    },
    WithdrawRewards {
        amount: Uint128,
        status: ResponseStatus,
    },
    Redelegate {
        amount: Uint128,
        status: ResponseStatus,
    },
    // ............
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    IdoAmount {},
    IdoInfo {
        ido_id: u32,
    },
    InWhitelist {
        address: String,
        ido_id: u32,
    },
    IdoListOwnedBy {
        address: String,
        start: u32,
        limit: u32,
    },
    Purchases {
        ido_id: u32,
        address: String,
        start: Option<u32>,
        limit: Option<u32>,
    },
    ArchivedPurchases {
        ido_id: u32,
        address: String,
        start: u32,
        limit: u32,
    },
    UserInfo {
        address: String,
        ido_id: Option<u32>,
    },
    TierUserInfo {
        address: String,
    },
    Withdrawals {
        address: String,
        start: Option<u32>,
        limit: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PurchaseAnswer {
    pub tokens_amount: Uint128,
    pub timestamp: u64,
    pub unlock_time: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SerializedWithdrawals {
    pub amount: Uint128,
    pub claim_time: u64,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResponse {
    Config {
        admin: String,
        nft_contract: String,
        lock_periods: Vec<u64>,
        validators: Vec<ValidatorWithWeight>,
        status: u8,
        usd_deposits: Vec<Uint128>,
        min_tier: u8,
    },
    IdoAmount {
        amount: u32,
    },
    IdoInfo {
        admin: String,
        start_time: u64,
        end_time: u64,
        token_contract: String,
        price: Uint128,
        participants: u64,
        payment: PaymentMethod,
        sold_amount: Uint128,
        total_tokens_amount: Uint128,
        total_payment: Uint128,
        soft_cap: Uint128,
        withdrawn: bool,
        shared_whitelist: bool,
        remaining_per_tiers: Vec<Uint128>,
    },
    InWhitelist {
        in_whitelist: bool,
    },
    IdoListOwnedBy {
        ido_ids: Vec<u32>,
        amount: u32,
    },
    Purchases {
        purchases: Vec<PurchaseAnswer>,
        amount: u32,
    },
    ArchivedPurchases {
        purchases: Vec<PurchaseAnswer>,
        amount: u32,
    },
    UserInfo {
        total_payment: Uint128,
        total_tokens_bought: Uint128,
        total_tokens_received: Uint128,
    },
    TierUserInfo {
        tier: u8,
        timestamp: u64,
        usd_deposit: Uint128,
        orai_deposit: Uint128,
    },
    TierInfo {
        tier: u8,
        nft_tier: u8,
    },
    Withdrawals {
        amount: u32,
        withdrawals: Vec<SerializedWithdrawals>,
    },
}
