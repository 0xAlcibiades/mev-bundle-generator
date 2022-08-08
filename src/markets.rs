use std::collections::{HashMap, HashSet};
/// Eth market traits and interfaces
use std::fmt;
use std::ops::{Deref, Not};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use log::info;
use petgraph::graphmap::UnGraphMap;
use web3::transports::WebSocket;
use web3::types::{Address, Log, U256};
use web3::Web3;

use crate::address_book;
use crate::compound;
use crate::evm::Call;
use crate::uniswap;
use crate::uniswap::UniswapV2Pair;
use crate::weth_token;
use rayon::prelude::*;

#[derive(Clone, Copy, Debug)]
/// An enum of protocols
pub enum Protocol {
    UniswapV2,
    ERC20,
    Compound,
}

#[derive(Debug)]
pub(crate) enum TokenInputError {
    InvalidToken,
}

impl std::error::Error for TokenInputError {}

impl fmt::Display for TokenInputError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TokenInputError::InvalidToken => write!(f, "Bad token address"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// A Token Pair for an market
pub struct TokenPair {
    pub(crate) i: Address,
    pub(crate) j: Address,
}

/// A trait for types of ethereum token markets
#[async_trait]
pub trait Market: Send + Sync {
    /// Return the i and j tokens for a market
    fn tokens(&self) -> TokenPair;

    /// Return the address for a market
    fn market_address(&self) -> Address;

    // Contracts to monitor for delta updates
    fn delta_contracts(&self) -> Vec<Address>;

    /// Return the protocol for a market
    fn protocol(&self) -> Protocol;

    /// Return the default miner payment for this market
    fn miner_reward_percentage(&self) -> Option<U256>;

    /// Get the tokens out for a given token amount in.
    fn get_tokens_out(&self, token_in: &Address, token_out: &Address, amount_in: &U256) -> U256;

    /// Get the tokens in for a given amount out.
    fn get_tokens_in(&self, token_in: &Address, token_out: &Address, amount_out: &U256) -> U256;

    /// Should this return CallData
    fn sell_tokens(
        &self,
        token_in: &Address,
        amount_in: &U256,
        recipient: &Address,
    ) -> Result<Vec<Call>>;

    /// Should update any info from the last state block which needs to be updated
    ///
    /// This is async because it operates on state data from the blockchain.
    ///
    /// Each market should be able to update itself as needed.
    async fn update(&mut self);

    /// Can this market receive a token directly?
    fn receive_directly(&self, token_address: &Address) -> bool;

    /// Generate calls for first market in chain
    fn to_first_market(&self, token_address: &Address, amount: &U256) -> Result<Option<Vec<Call>>>;

    /// Generate calls, such as token approvals
    fn prepare_receive(&self, token_address: &Address) -> Result<Option<Vec<Call>>>;
}

/// Token Markets, for an edge in the graph of markets

pub struct TokenMarkets {
    pub markets: Vec<Box<dyn Market>>,
}

impl fmt::Debug for TokenMarkets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenMarkets")
            .field("markets", &self.markets.len())
            .finish()
    }
}

impl TokenMarkets {
    pub fn new() -> TokenMarkets {
        TokenMarkets { markets: vec![] }
    }

    // TODO(Handle what clippy says here around a boxed reference)
    /// Return the best market to sell to, given an origin in the token pair
    pub fn best_bid_market(
        &self,
        origin: &Address,
        destination: &Address,
        amount_out: &U256,
    ) -> (&dyn Market, U256) {
        // Get the least tokens in for the same amount out
        let mut best_bid_market = &self.markets[0];
        let mut best_bid = best_bid_market.get_tokens_in(origin, destination, amount_out);
        for (idx, market) in self.markets.iter().enumerate() {
            let bid = market.get_tokens_in(origin, destination, amount_out);
            if bid < best_bid {
                best_bid_market = &self.markets[idx];
                best_bid = bid;
            }
        }
        (best_bid_market.deref(), best_bid)
    }

    /// Return the best market to buy from, given an origin in the token pair
    pub fn best_ask_market(
        &self,
        origin: &Address,
        destination: &Address,
        amount_in: &U256,
    ) -> (&dyn Market, U256) {
        // Get the most tokens out for the same tokens in
        let mut best_ask_market = &self.markets[0];
        let mut best_offer = best_ask_market.get_tokens_out(origin, destination, amount_in);
        for (idx, market) in self.markets.iter().enumerate() {
            let offer = market.get_tokens_out(origin, destination, amount_in);
            if offer > best_offer {
                best_ask_market = &self.markets[idx];
                best_offer = offer;
            }
        }
        (best_ask_market.deref(), best_offer)
    }

    pub async fn update(&mut self) {
        // TODO(This should only update markets with deltas based on events from the previous block)
        let mut updates = vec![];
        for market in self.markets.iter_mut() {
            updates.push(market.update());
        }
        futures::future::join_all(updates).await;
    }

    pub fn market_count(&self) -> usize {
        self.markets.len()
    }
}

// TODO(Consider if this should be a directed graph with parallel edges, rather than undirected)
// TokenMarkets provides pseudo direction.

#[derive(Debug)]
pub struct MarketGraph {
    // TODO(Add all markets to graph)
    pub graph: UnGraphMap<Address, TokenMarkets>,
    // All cycles by origin token
    pub cycles_by_token: HashMap<Address, Vec<Vec<Address>>>,
}

impl MarketGraph {
    pub async fn new(transport: &Web3<WebSocket>) -> MarketGraph {
        // Gather all markets
        info!("Gathering markets.");

        // Gather uniswap V2 markets
        let mut graph = UnGraphMap::<Address, TokenMarkets>::with_capacity(15000, 15000);
        let par_graph = Arc::new(Mutex::new(&mut graph));
        let v2_markets: Vec<UniswapV2Pair> = uniswap::UniswapV2Pair::get_all_markets(transport)
            .await
            .unwrap();
        let v2_market_count = v2_markets.len();
        info!("Gathered {} Uniswap V2 Like Markets", v2_market_count);

        // TODO(This might not benefit from parallelization, just an experiment with rayon)
        // Add all v2 markets to graph
        let _x: () = v2_markets
            .into_par_iter()
            .map(|market| {
                let token_i = market.tokens().i;
                let token_j = market.tokens().j;
                let mut locked_graph = par_graph.lock().unwrap();
                // Do I need to check if these exist first?
                if locked_graph.contains_node(token_i).not() {
                    locked_graph.add_node(token_i);
                }
                if locked_graph.contains_node(token_j).not() {
                    locked_graph.add_node(token_j);
                }
                if locked_graph
                    .contains_edge(market.tokens().i, market.tokens().j)
                    .not()
                {
                    locked_graph.add_edge(token_i, token_j, TokenMarkets::new());
                }
                // Get a mutable reference to the edge
                let edge = locked_graph.edge_weight_mut(token_i, token_j).unwrap();
                edge.markets.push(Box::new(market))
            })
            .collect();

        // weth <-> eth
        let eth: Address = address_book::ETH_ADDRESS.parse().unwrap();
        let weth: Address = address_book::WETH_ADDRESS.parse().unwrap();
        if graph.contains_node(eth).not() {
            graph.add_node(eth);
        }
        // Unlikely, but why not check?
        if graph.contains_node(weth).not() {
            graph.add_node(weth);
        }
        if graph.contains_edge(eth, weth).not() {
            graph.add_edge(eth, weth, TokenMarkets::new());
        }
        let market = weth_token::WethEthMarket::new(transport);
        let edge = graph.edge_weight_mut(eth, weth).unwrap();
        edge.markets.push(Box::new(market));

        // ceth <-> eth
        let ceth: Address = address_book::CETH_ADDRESS.parse().unwrap();
        if graph.contains_node(ceth).not() {
            graph.add_node(ceth);
        }
        if graph.contains_edge(eth, ceth).not() {
            graph.add_edge(eth, ceth, TokenMarkets::new());
        }
        let market = compound::CethEthMarket::new(transport);
        let edge = graph.edge_weight_mut(eth, ceth).unwrap();
        edge.markets.push(Box::new(market));

        // Return market.
        info!(
            "Constructed market graph with {} tokens trading on {} markets with {} token markets.",
            graph.node_count(),
            v2_market_count + 1,
            graph.edge_count()
        );
        let cycles_by_token: HashMap<Address, Vec<Vec<Address>>> = HashMap::new();
        MarketGraph {
            graph,
            cycles_by_token,
        }
    }

    pub fn total_market_count(&self) -> usize {
        self.graph.all_edges().map(|tm| tm.2.market_count()).sum()
    }

    /// Update markets based on the state block logs
    pub async fn update_delta(&mut self, state_block_logs: &[Log]) {
        let always_update: Address = address_book::MulticallEXECUTOR.parse().unwrap();
        // Get a list of addresses with deltas
        let mut delta_addresses = HashSet::new();
        for log in state_block_logs.iter() {
            delta_addresses.insert(log.address);
        }
        let edges = self.graph.all_edges_mut();
        let mut updates = vec![];
        // TODO(A hash map of references to markets could take this from O(n^3) to O(1))
        // Rust doesn't make storing a self ref easy, can research later.
        for tm in edges {
            for market in tm.2.markets.iter_mut() {
                let mut update = false;
                let contract_addresses = market.delta_contracts();
                for contract_address in contract_addresses {
                    if contract_address.0 == always_update.0 {
                        // The bundle executor address means we should update every block
                        update = true
                    }
                    if delta_addresses.contains(&contract_address) {
                        update = true
                    }
                }
                if update {
                    updates.push(market.update());
                }
            }
        }
        info!(
            "Updating {} markets with deltas in the last state block.",
            updates.len()
        );
        futures::future::join_all(updates).await;
    }

    /// Update all markets
    pub async fn update_all(&mut self) {
        // TODO(Do we need to add or remove any markets here?)
        info!("Updating {} markets.", self.total_market_count());
        let mut token_market_updates = vec![];
        for tm in self.graph.all_edges_mut() {
            token_market_updates.push(tm.2.update());
        }
        futures::future::join_all(token_market_updates).await;
    }
}
