use anyhow::Error;
use async_trait::async_trait;
use web3::ethabi::ethereum_types::U256;
use web3::types::{Address, BlockId, BlockNumber};

use crate::constants;
use crate::evm;
use crate::evm::Call;
use crate::markets::{Market, Protocol, TokenPair};
use crate::{address_book, markets};
use web3::contract::tokens::Tokenize;
use web3::contract::Contract;
use web3::transports::WebSocket;
use web3::Web3;

/// A market for weth <-> eth
pub struct CethEthMarket {
    tokens: TokenPair,
    bundle_executor: Address,
    ceth: Contract<WebSocket>,
    exchange_rate: U256,
}

impl CethEthMarket {
    pub fn new(transport: &Web3<WebSocket>) -> CethEthMarket {
        let eth: Address = address_book::ETH_ADDRESS.parse().unwrap();
        let ceth: Address = address_book::CETH_ADDRESS.parse().unwrap();
        let ceth_contract = Contract::from_json(
            transport.eth(),
            ceth,
            include_bytes!("protocols/compound/ceth.json"),
        )
        .unwrap();
        let bundle_executor: Address = address_book::MulticallEXECUTOR.parse().unwrap();
        CethEthMarket {
            tokens: TokenPair { i: eth, j: ceth },
            bundle_executor,
            ceth: ceth_contract,
            exchange_rate: constants::ZERO_U256,
        }
    }

    async fn update_exchange_rate(&mut self) {
        // There is really no easy way to cache this without duplicating all the interest
        // accrual logic for compound, which might be worth considering later.
        // The exchange rate with eth is scaled by 10^18
        self.exchange_rate = self
            .ceth
            .query::<U256, _, _, _>(
                "exchangeRateCurrent",
                (),
                self.bundle_executor,
                Default::default(),
                BlockId::from(BlockNumber::Pending),
            )
            .await
            .unwrap()
    }
}

#[async_trait]
impl Market for CethEthMarket {
    fn tokens(&self) -> TokenPair {
        self.tokens
    }

    fn market_address(&self) -> Address {
        self.bundle_executor
    }

    fn delta_contracts(&self) -> Vec<Address> {
        // No updates to be made here.
        return vec![self.bundle_executor];
    }

    fn protocol(&self) -> Protocol {
        Protocol::Compound
    }

    fn miner_reward_percentage(&self) -> Option<U256> {
        None
    }

    // These functions are essentially inverse operations
    fn get_tokens_out(&self, token_in: &Address, token_out: &Address, amount_in: &U256) -> U256 {
        if token_in.0 == self.tokens.i.0 && token_out.0 == self.tokens.j.0 {
            // Token in is Eth
            amount_in / self.exchange_rate
        } else if token_in.0 == self.tokens.j.0 && token_out.0 == self.tokens.i.0 {
            // Token in is Ceth
            amount_in * self.exchange_rate
        } else {
            constants::ZERO_U256
        }
    }

    fn get_tokens_in(&self, token_in: &Address, token_out: &Address, amount_out: &U256) -> U256 {
        if token_in.0 == self.tokens.i.0 && token_out.0 == self.tokens.j.0 {
            // Token in is Eth
            amount_out * self.exchange_rate
        } else if token_in.0 == self.tokens.j.0 && token_out.0 == self.tokens.i.0 {
            // Token in is Ceth
            amount_out / self.exchange_rate
        } else {
            constants::ZERO_U256
        }
    }

    fn sell_tokens(
        &self,
        token_in: &Address,
        amount_in: &U256,
        _recipient: &Address,
    ) -> anyhow::Result<Vec<Call>> {
        // TODO(Implement recipient)
        // This involves adding value call encoding support
        if token_in.0 == self.tokens.i.0 {
            // Eth
            // TODO()
            // Mint eth -> ceth, transfer if recipient != address
            let params = ();
            let raw_call = self
                .ceth
                .abi()
                .function("mint")
                .unwrap()
                .encode_input(&params.into_tokens())
                .unwrap();
            let calls = Call::new(
                self.ceth.address(),
                raw_call[0..4].to_vec(),
                evm::Type::ValueCall,
                Option::from(*amount_in),
                raw_call[4..].to_vec(),
            );
            Ok(vec![calls])
        } else if token_in.0 == self.tokens.j.0 {
            // Ceth
            // Redeem ceth -> eth,
            let params = *amount_in;
            let raw_call = self
                .ceth
                .abi()
                .function("redeem")
                .unwrap()
                .encode_input(&params.into_tokens())
                .unwrap();
            let calls = Call::new(
                self.ceth.address(),
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
        self.update_exchange_rate().await
    }

    fn receive_directly(&self, _token_address: &Address) -> bool {
        // Yes because the market is the executor
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
