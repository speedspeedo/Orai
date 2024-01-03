use cosmwasm_std::{Decimal, StdResult};

use cosmwasm_std::DepsMut;

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
        let rate = 8123456;
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
