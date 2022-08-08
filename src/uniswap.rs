use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Error, Result};
use async_trait::async_trait;
use rayon::prelude::*;
use web3::api::Eth;
use web3::contract::tokens::Tokenize;
use web3::contract::{Contract, Options};
use web3::ethabi::Uint;
use web3::transports::WebSocket;
use web3::types::{Address, BlockId, BlockNumber, U256};
use web3::Web3;

use crate::evm::Call;
use crate::markets::{Market, Protocol, TokenPair};
use crate::{address_book, constants, evm, markets};
use std::sync::{Arc, Mutex};

const BATCH_COUNT_LIMIT: u32 = 250;
const UNISWAP_BATCH_SIZE: u32 = 250;

#[derive(Debug, Clone)]
pub struct Router {
    contract: Contract<WebSocket>,
}

impl Router {
    pub fn new(address: Address, transport: &web3::Web3<WebSocket>) -> Router {
        let contract: Contract<WebSocket> = Contract::from_json(
            transport.eth(),
            address,
            include_bytes!("protocols/uniswap/v2/abis/router.json"),
        )
        .unwrap();
        Router { contract }
    }

    /// Uses Uniswap v2 getAmountsOut to get the price in weth of a token.
    pub async fn get_price_wei(self, token: &Address) -> U256 {
        // TODO(Make this more robust)
        let path = vec![
            *token,
            Address::from_str(address_book::WETH_ADDRESS).unwrap(),
        ];
        let price = self
            .contract
            .query::<Vec<U256>, _, _, _>(
                "getAmountsOut",
                (constants::ETHER, path),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await;
        return price.unwrap()[1];
    }
}

#[derive(Clone, Debug)]
pub struct UniswapV2Pair {
    uniswap_interface: Contract<WebSocket>,
    eth: Eth<WebSocket>,
    token_balances: HashMap<Address, U256>,
    market_address: Address,
    tokens: TokenPair,
    protocol: Protocol,
}

impl UniswapV2Pair {
    pub fn new(
        transport: &Web3<WebSocket>,
        market_address: Address,
        tokens: TokenPair,
    ) -> UniswapV2Pair {
        // TODO(Do we really always need a copy of this in memory?)
        let uniswap_interface = Contract::from_json(
            transport.eth(),
            market_address,
            include_bytes!("protocols/uniswap/v2/abis/pair.json"),
        )
        .unwrap();
        let mut token_balances = HashMap::new();
        token_balances.insert(tokens.i, constants::ZERO_U256);
        token_balances.insert(tokens.j, constants::ZERO_U256);
        UniswapV2Pair {
            uniswap_interface,
            eth: transport.eth(),
            token_balances,
            market_address,
            tokens,
            protocol: Protocol::UniswapV2,
        }
    }

    pub fn get_amount_in(reserve_in: &U256, reserve_out: &U256, amount_out: &U256) -> U256 {
        if reserve_out < amount_out {
            // Catch overflow
            return constants::ZERO_U256;
        }
        let numerator = (reserve_in * amount_out) * 1000;
        let denominator = (reserve_out - amount_out) * 997;
        if numerator == constants::ZERO_U256 || denominator == constants::ZERO_U256 {
            return constants::ZERO_U256;
        }
        (numerator / denominator) + constants::ONE_U256
    }

    pub fn get_amount_out(reserve_in: &U256, reserve_out: &U256, amount_in: &U256) -> U256 {
        // TODO(Seems like we could do this better, this will lose data with large amounts)
        let amount_in_with_fee = amount_in * 997;
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = (reserve_in * 1000) + amount_in_with_fee;
        if numerator == constants::ZERO_U256 || denominator == constants::ZERO_U256 {
            return constants::ZERO_U256;
        }
        numerator / denominator
    }

    async fn get_uniswappy_markets_batch(
        transport: &Web3<WebSocket>,
        query_interface: &Contract<WebSocket>,
        factory_address: &Address,
        start: u32,
        stop: u32,
    ) -> Vec<UniswapV2Pair> {
        let mut batch_markets = vec![];
        let par_batch_markets = Arc::new(Mutex::new(&mut batch_markets));
        let batch_pairs = query_interface
            .query::<Vec<Vec<Address>>, _, _, _>(
                "getPairsByIndexRange",
                (*factory_address, start, stop),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();

        let _x: () = batch_pairs
            .into_par_iter()
            .map(|pair| {
                let mut push = true;
                let market_address = pair[2];
                let i = pair[0];
                let j = pair[1];
                for token in address_book::BLACKLISTED_TOKENS.iter() {
                    // TODO(Make this faster by figuring out how to store all the addresses as constants)
                    // Also, could use hashmap here
                    let token: Address = token.parse().unwrap();
                    if (j.0 == token.0) || (i.0 == token.0) {
                        push = false;
                    }
                }
                for bad_pool in address_book::BLACKLISTED_POOLS.iter() {
                    let bad: Address = bad_pool.parse().unwrap();
                    if pair[2].0 == bad.0 {
                        push = false
                    }
                }
                if push {
                    let eth_pair =
                        UniswapV2Pair::new(transport, market_address, TokenPair { i, j });
                    par_batch_markets.lock().unwrap().push(eth_pair);
                }
            })
            .collect();
        batch_markets
    }

    //pub fn get_balance(&self, token: &Address) -> U256 {
    //    self.token_balances[token]
    //}

    /// Return a list of all weth market pairs for a given uniswap v2 factory address
    pub async fn get_uniswappy_markets(
        transport: &Web3<WebSocket>,
        factory_address: &Address,
    ) -> Result<Vec<UniswapV2Pair>> {
        let uniswap_query_interface = Contract::from_json(
            transport.eth(),
            address_book::UNISWAP_LOOKUP_CONTRACT_ADDRESS
                .parse()
                .unwrap(),
            include_bytes!("abis/UniswapV2FlashQuery.json"),
        )
        .unwrap();
        let uniswap_factory_interface = Contract::from_json(
            transport.eth(),
            *factory_address,
            include_bytes!("protocols/uniswap/v2/abis/factory.json"),
        )
        .unwrap();
        let num_pairs = uniswap_factory_interface
            .query::<U256, _, _, _>(
                "allPairsLength",
                (),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();
        let mut batch_futures = vec![];
        let mut pos = 0_u32;
        'batches: for _ in 0..BATCH_COUNT_LIMIT {
            // Do the things per batch
            let start = pos;
            let stop = pos + UNISWAP_BATCH_SIZE;
            batch_futures.push(UniswapV2Pair::get_uniswappy_markets_batch(
                transport,
                &uniswap_query_interface,
                factory_address,
                start,
                stop,
            ));
            pos += UNISWAP_BATCH_SIZE;
            if num_pairs < U256::from(stop) {
                break 'batches;
            }
        }
        let market_pairs: Vec<UniswapV2Pair> = futures::future::join_all(batch_futures)
            .await
            .into_iter()
            .flatten()
            .collect();
        Ok(market_pairs)
    }

    pub async fn get_all_markets(transport: &Web3<WebSocket>) -> Result<Vec<UniswapV2Pair>> {
        let factory_addresses: Vec<Address> = address_book::FACTORY_ADDRESSES
            .iter()
            .map(|address| address.parse().unwrap())
            .collect();
        let mut all_pairs: Vec<Vec<UniswapV2Pair>> = vec![];
        for factory in factory_addresses {
            all_pairs.push(
                UniswapV2Pair::get_uniswappy_markets(transport, &factory)
                    .await
                    .unwrap(),
            );
        }
        let all_pairs: Vec<UniswapV2Pair> = all_pairs.into_iter().flatten().collect();
        Ok(all_pairs)
    }
}

#[async_trait]
impl Market for UniswapV2Pair {
    fn tokens(&self) -> TokenPair {
        self.tokens
    }

    fn market_address(&self) -> Address {
        self.market_address
    }

    fn delta_contracts(&self) -> Vec<Address> {
        vec![self.market_address]
    }

    fn protocol(&self) -> Protocol {
        self.protocol
    }

    fn miner_reward_percentage(&self) -> Option<U256> {
        Some(U256::from(99))
    }

    fn get_tokens_out(&self, token_in: &Address, token_out: &Address, amount_in: &U256) -> U256 {
        let reserve_in = self.token_balances[token_in];
        let reserve_out = self.token_balances[token_out];
        UniswapV2Pair::get_amount_out(&reserve_in, &reserve_out, amount_in)
    }

    fn get_tokens_in(&self, token_in: &Address, token_out: &Address, amount_out: &U256) -> U256 {
        let reserve_in = self.token_balances[token_in];
        let reserve_out = self.token_balances[token_out];
        UniswapV2Pair::get_amount_in(&reserve_in, &reserve_out, amount_out)
    }

    fn sell_tokens(
        &self,
        token_in: &Address,
        amount_in: &U256,
        recipient: &Address,
    ) -> Result<Vec<Call>> {
        let mut amount_0_out = constants::ZERO_U256;
        let mut amount_1_out = constants::ZERO_U256;
        if token_in.0 == self.tokens.i.0 {
            let token_out = self.tokens.j;
            amount_1_out = self.get_tokens_out(token_in, &token_out, amount_in);
        } else if token_in.0 == self.tokens.j.0 {
            let token_out = self.tokens.i;
            amount_0_out = self.get_tokens_out(token_in, &token_out, amount_in);
        } else {
            return Err(Error::from(markets::TokenInputError::InvalidToken));
        }
        let data: Vec<u8> = vec![];
        let params = (amount_0_out, amount_1_out, *recipient, data);
        let raw_call = self
            .uniswap_interface
            .abi()
            .function("swap")
            .unwrap()
            .encode_input(&params.into_tokens())
            .unwrap();
        // Only one call needed here.
        let calls = Call::new(
            self.uniswap_interface.address(),
            raw_call[0..4].to_vec(),
            evm::Type::Call,
            None,
            raw_call[4..].to_vec(),
        );
        Ok(vec![calls])
    }

    async fn update(&mut self) {
        let reserves: (Uint, Uint, Uint) = self
            .uniswap_interface
            .query(
                "getReserves",
                (),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();
        self.token_balances.insert(self.tokens.i, reserves.0);
        self.token_balances.insert(self.tokens.j, reserves.1);
    }

    fn receive_directly(&self, token_address: &Address) -> bool {
        self.token_balances.contains_key(token_address)
    }

    fn to_first_market(&self, token_address: &Address, amount: &U256) -> Result<Option<Vec<Call>>> {
        // Generate contract
        let token = Contract::from_json(
            self.eth.clone(),
            *token_address,
            include_bytes!("abis/IERC20.json"),
        )
        .unwrap();
        let raw_call = token
            .abi()
            .function("transfer")
            .unwrap()
            .encode_input(&(self.uniswap_interface.address(), *amount).into_tokens())
            .unwrap();
        let calls = Call::new(
            *token_address,
            raw_call[0..4].to_vec(),
            evm::Type::Call,
            None,
            raw_call[4..].to_vec(),
        );
        Ok(Option::from(vec![calls]))
    }

    // This is a noop for Uniswap V2
    fn prepare_receive(&self, _token_address: &Address) -> Result<Option<Vec<Call>>> {
        Ok(None)
    }
}
