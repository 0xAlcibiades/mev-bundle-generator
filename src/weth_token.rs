use anyhow::Error;
use async_trait::async_trait;
use web3::ethabi::ethereum_types::U256;
use web3::types::Address;

use crate::evm;
use crate::evm::Call;
use crate::markets::{Market, Protocol, TokenPair};
use crate::{address_book, markets};
use web3::contract::tokens::Tokenize;
use web3::contract::Contract;
use web3::transports::WebSocket;
use web3::Web3;

/// A market for weth <-> eth
pub struct WethEthMarket {
    tokens: TokenPair,
    bundle_executor: Address,
    weth: Contract<WebSocket>,
}

impl WethEthMarket {
    pub fn new(transport: &Web3<WebSocket>) -> WethEthMarket {
        let eth: Address = address_book::ETH_ADDRESS.parse().unwrap();
        let weth: Address = address_book::WETH_ADDRESS.parse().unwrap();
        let weth_contract =
            Contract::from_json(transport.eth(), weth, include_bytes!("abis/WETH9.json")).unwrap();
        let bundle_executor: Address = address_book::MulticallEXECUTOR.parse().unwrap();
        WethEthMarket {
            tokens: TokenPair { i: eth, j: weth },
            bundle_executor,
            weth: weth_contract,
        }
    }
}

#[async_trait]
impl Market for WethEthMarket {
    fn tokens(&self) -> TokenPair {
        self.tokens
    }

    fn market_address(&self) -> Address {
        self.bundle_executor
    }

    fn delta_contracts(&self) -> Vec<Address> {
        // No updates to be made here.
        return vec![];
    }

    fn protocol(&self) -> Protocol {
        Protocol::ERC20
    }

    fn miner_reward_percentage(&self) -> Option<U256> {
        None
    }

    fn get_tokens_out(&self, _token_in: &Address, _token_out: &Address, amount_in: &U256) -> U256 {
        // This is 1:1
        *amount_in
    }

    fn get_tokens_in(&self, _token_in: &Address, _token_out: &Address, amount_out: &U256) -> U256 {
        // This is 1:1
        *amount_out
    }

    fn sell_tokens(
        &self,
        token_in: &Address,
        amount_in: &U256,
        _recipient: &Address,
    ) -> anyhow::Result<Vec<Call>> {
        // TODO(Implement transfer to recipient)
        // TODO(Add calls here)
        // This involves adding value call encoding support
        if token_in.0 == self.tokens.i.0 {
            // Eth
            // TODO()
            // Deposit eth -> weth, transfer if recipient != address
            let params = ();
            let raw_call = self
                .weth
                .abi()
                .function("deposit")
                .unwrap()
                .encode_input(&params.into_tokens())
                .unwrap();
            let calls = Call::new(
                self.weth.address(),
                raw_call[0..4].to_vec(),
                evm::Type::ValueCall,
                Option::from(*amount_in),
                raw_call[4..].to_vec(),
            );
            Ok(vec![calls])
        } else if token_in.0 == self.tokens.j.0 {
            // Weth
            // Withdraw weth -> eth,
            let params = *amount_in;
            let raw_call = self
                .weth
                .abi()
                .function("withdraw")
                .unwrap()
                .encode_input(&params.into_tokens())
                .unwrap();
            let calls = Call::new(
                self.weth.address(),
                raw_call[0..4].to_vec(),
                evm::Type::Call,
                None,
                raw_call[4..].to_vec(),
            );
            Ok(vec![calls])
        } else {
            Err(Error::from(markets::TokenInputError::InvalidToken))
        }
    }

    async fn update(&mut self) {
        // This is a noop, always 1:1
    }

    fn receive_directly(&self, _token_address: &Address) -> bool {
        true
    }

    // TODO(These could be implemented to allow management of funds in another contract or EOA)
    fn to_first_market(
        &self,
        _token_address: &Address,
        _amount: &U256,
    ) -> anyhow::Result<Option<Vec<Call>>> {
        Ok(None)
    }

    fn prepare_receive(&self, _token_address: &Address) -> anyhow::Result<Option<Vec<Call>>> {
        Ok(None)
    }
}
