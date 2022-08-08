use web3::{
    contract::{Contract, Options},
    transports::WebSocket,
    types::{Address, U256},
};

use crate::address_book;

#[derive(Debug, Clone)]
pub struct MasterChef {
    contract: Contract<WebSocket>,
}

impl MasterChef {
    pub fn new(transport: &web3::Web3<WebSocket>) -> MasterChef {
        // TODO(Consider loading dynamically)
        // Or moving to the string to addy_book
        let contract: Contract<WebSocket> = Contract::from_json(
            transport.eth(),
            address_book::SUSHI_MASTER_CHEF.parse().unwrap(),
            include_bytes!("protocols/sushiswap/abis/sushi_chef.json"),
        )
        .unwrap();
        MasterChef { contract }
    }

    pub async fn pending_reward_balance(&self, pid: U256, address: Address) -> U256 {
        self.contract
            .query::<U256, _, _, _>(
                "pendingSushi",
                (pid, address),
                None,
                Options::default(),
                None,
            )
            .await
            .unwrap()
    }
}
