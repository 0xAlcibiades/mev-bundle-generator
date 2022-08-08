use log::debug;

use web3::transports::WebSocket;
use web3::types::U256;
use web3::Web3;

#[derive(Debug, Clone, Copy)]
pub struct GasPrice {
    pub ludicrous: U256,
    pub high: U256,
    pub medium: U256,
    pub low: U256,
}

impl GasPrice {
    async fn get_prices(transport: &Web3<WebSocket>) -> (U256, U256, U256, U256) {
        // TODO(This should just get a reference to the txpool)
        let block_info = transport.txpool().content().await;
        debug!("Building gas pricing from txpool");
        let mut gas_prices = vec![];
        match block_info {
            Err(_err) => (),
            Ok(info) => {
                let pending_txpool = info.pending;
                // TODO (This is a naive algo O(n^2), there must be a better way)
                for address in &pending_txpool {
                    for tx in address.1 {
                        gas_prices.push(tx.1.gas_price)
                    }
                }
            }
        }
        if gas_prices.is_empty() {
            // Fallback pricing for no txpool.
            let estimated = transport.eth().gas_price().await.unwrap();
            return (
                (estimated + estimated),
                (&estimated + 4),
                (&estimated + 2),
                estimated,
            );
        } else {
            gas_prices.sort();
            gas_prices.reverse();
            // TODO(Make this update to the largest in the last 1000 blocks dynamically)
            gas_prices.truncate(250);
            let high = gas_prices.first().unwrap().to_owned();
            let medium = gas_prices.get(gas_prices.len() / 2).unwrap().to_owned();
            let low = gas_prices.last().unwrap().to_owned();
            let ludicrous = &high * 3_u64;
            (ludicrous, high, medium, low)
        }
    }

    pub async fn new(transport: &Web3<WebSocket>) -> GasPrice {
        let (ludicrous, high, medium, low) = GasPrice::get_prices(transport).await;
        GasPrice {
            ludicrous,
            high,
            medium,
            low,
        }
    }
}
