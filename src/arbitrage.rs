use std::fmt;

use async_trait::async_trait;
use log::{debug, info};
use web3::contract::{Contract, Options};
use web3::transports::WebSocket;
use web3::types::{Address, BlockId, BlockNumber, TransactionParameters, U256, U64};
use web3::{ethabi, Web3};

use crate::constants::{ETHER, FINNEY};
use crate::evm::{Call, Multicall, MulticallHeader};
use crate::flashbots::{Bundle, BundleGenerator};
use crate::gas::GasPrice;
use crate::markets::{Market, MarketGraph};
use crate::utilities::Transaction;
use crate::{address_book, constants, utilities};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use web3::contract::tokens::Tokenize;

/// Details about a crossed bid/ask market.
#[derive(Clone, Debug)]
pub struct CrossedMarketDetails<'a, T: Market + ?Sized> {
    profit: U256,
    volume: U256,
    origin_token: Address,
    intermediary_token: Address,
    ask_market: &'a T,
    bid_market: &'a T,
}

/// Implementation of crossed market details for an ethmarket
impl<'a, T: Market + ?Sized> CrossedMarketDetails<'a, T> {
    pub fn new(
        origin_token: Address,
        intermediary_token: Address,
        bid_market: &'a T,
        ask_market: &'a T,
    ) -> CrossedMarketDetails<'a, T> {
        let profit = constants::ZERO_U256;
        let volume = constants::ZERO_U256;
        CrossedMarketDetails {
            profit,
            volume,
            origin_token,
            intermediary_token,
            ask_market,
            bid_market,
        }
    }

    /// Optimize order volume for profit
    pub fn optimize_volume(&self) -> (U256, U256) {
        // First a log test sizing to isolate the maxima
        // TODO(Figure out a way to do this without a log scale to reduce iterations)
        let test_sizes = vec![
            FINNEY * 10,
            FINNEY * 100,
            FINNEY * 200,
            FINNEY * 300,
            FINNEY * 400,
            FINNEY * 500,
            FINNEY * 600,
            FINNEY * 700,
            FINNEY * 800,
            FINNEY * 900,
            ETHER,
            ETHER * 2,
            ETHER * 3,
            ETHER * 4,
            ETHER * 5,
            ETHER * 6,
            ETHER * 7,
            ETHER * 8,
            ETHER * 9,
            ETHER * 10,
            ETHER * 20,
            ETHER * 30,
            ETHER * 40,
            ETHER * 50,
        ];
        let mut low = test_sizes[0];
        let mut high = test_sizes[2];
        for i in 1..(test_sizes.len() - 2) {
            if self.order_profit(&test_sizes[i]) > self.order_profit(&test_sizes[i - 1]) {
                low = test_sizes[i - 1];
                high = test_sizes[i + 1];
            } else {
                // Then we've found the sweet spot.
                break;
            }
        }
        let mut mid = (high + low) / 2;
        // Here we optimize the size of a crossed market order for profit.
        // Binary could take as many as 256
        // iterations, and much less than that in the bounded search space.
        // https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.303.2069&rep=rep1&type=pdf
        // A quadratic search could do it 2x faster
        // TODO(Implement quadratic search)
        let mut profit_low;
        let mut profit_high;
        let mut profit_mid;
        let mut profit_step_up;
        let mut profit_step_down;
        // TODO(Size these correctly, I am losing orders on the previous version because of )
        // imprecision.
        // This is the step size for volumes to use in our search
        let step = U256::from(1000000000_u64);
        // Because this is a profit precision, it can be smaller
        let precision = U256::from(10_u64);
        // Binary gradient ascent with precision?
        loop {
            profit_low = self.order_profit(&low);
            profit_mid = self.order_profit(&mid);
            profit_high = self.order_profit(&high);
            // ABS delta, effectively - we have hit the desired precision
            if profit_high > profit_low && (profit_high - profit_low) < precision {
                break;
            }
            if profit_low > profit_high && (profit_low - profit_high) < precision {
                break;
            }
            // Let's get the gradients and ascend
            profit_step_up = self.order_profit(&(mid + step));
            profit_step_down = self.order_profit(&(mid - step));
            // Lets check the slope direction from midpoint
            if profit_mid < profit_step_up {
                // The slope is positive increasing volume
                low = mid;
            } else if profit_mid < profit_step_down {
                // The slope is positive decreasing volume
                high = mid;
            } else {
                // We are at the maxima
                break;
            };
            mid = (low + high) / 2;
        }
        (self.order_profit(&mid), mid)
    }

    pub fn order_profit(&self, order_size: &U256) -> U256 {
        let tokens_out = self.ask_market.get_tokens_out(
            &self.origin_token,
            &self.intermediary_token,
            order_size,
        );
        let proceeds = self.bid_market.get_tokens_out(
            &self.intermediary_token,
            &self.origin_token,
            &tokens_out,
        );
        if order_size < &proceeds {
            // The profits
            return proceeds - order_size;
        }
        // This is a hack to prevent underflow
        constants::ZERO_U256
    }
}

impl<'a, T: Market + ?Sized> fmt::Display for CrossedMarketDetails<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Profit: Ξ{} Volume: Ξ{} Token: {}\n Buy from: {}\n {} => {}\n Sell to: {} \n {} => {}\n\n",
               utilities::to_ether(&self.profit),
               utilities::to_ether(&self.volume),
               self.intermediary_token,
               self.ask_market.market_address(),
               self.origin_token,
               self.intermediary_token,
               self.bid_market.market_address(),
               self.intermediary_token,
               self.origin_token
        )
    }
}

#[derive(Debug, Clone)]
/// This engine finds simple a -> b -> a arbitrages
pub struct CrossedMarketArbitrageEngine {
    bundle_executor_contract: Contract<WebSocket>,
    ape_bank: Contract<WebSocket>,
}

impl CrossedMarketArbitrageEngine {
    pub async fn new(transport: &Web3<WebSocket>) -> CrossedMarketArbitrageEngine {
        let bundle_executor_contract = Contract::from_json(
            transport.eth(),
            address_book::MulticallEXECUTOR.parse().unwrap(),
            include_bytes!("abis/Multicall.json"),
        )
        .unwrap();
        let ape_bank = Contract::from_json(
            transport.eth(),
            address_book::APE_BANK.parse().unwrap(),
            include_bytes!("abis/ApeBank.json"),
        )
        .unwrap();
        CrossedMarketArbitrageEngine {
            bundle_executor_contract,
            ape_bank,
        }
    }

    pub fn evaluate_markets<'a>(
        &self,
        markets: &'a MarketGraph,
    ) -> Vec<CrossedMarketDetails<dyn Market + 'a>> {
        let mut crossed_markets: Vec<CrossedMarketDetails<dyn Market>> = vec![];
        let par_crossed_markets = Arc::new(Mutex::new(&mut crossed_markets));

        let cent = constants::FINNEY * 10;
        let mut edges = vec![];
        for address in address_book::ORIGIN_TOKENS {
            // TODO(Case here for test size based on decimal points).
            let origin: Address = address.parse().unwrap();

            for edge in markets.graph.edges(origin) {
                if edge.2.markets.len() >= 2 {
                    edges.push((edge.0, edge.1));
                }
            }
        }
        info!("Evaluating {} token pairs for arbitrage.", edges.len());
        let _x: () = edges
            .into_par_iter()
            .map(|edge| {
                // Can't have a crossed market without at least 2 markets
                let token_markets = markets.graph.edge_weight(edge.0, edge.1).unwrap();
                debug!(
                    "Searching for crossed markets between {} and {} with {} markets",
                    edge.0,
                    edge.1,
                    token_markets.markets.len()
                );
                // Buy tokens from origin
                let best_ask = token_markets.best_ask_market(&edge.0, &edge.1, &cent);
                // Sell tokens to get back to origin
                let best_bid = token_markets.best_bid_market(&edge.1, &edge.0, &cent);
                // If the output from buying is greater than the input for selling...
                if best_ask.1 > best_bid.1 {
                    let mut crossed_market =
                        CrossedMarketDetails::new(edge.0, edge.1, best_bid.0, best_ask.0);
                    let profit = crossed_market.order_profit(&cent);
                    crossed_market.profit = profit;
                    crossed_market.volume = cent;
                    let optimal_order = crossed_market.optimize_volume();
                    crossed_market.profit = optimal_order.0;
                    crossed_market.volume = optimal_order.1;
                    if crossed_market.profit > constants::FINNEY {
                        par_crossed_markets.lock().unwrap().push(crossed_market)
                    }
                }
            })
            .collect();
        // Sort best crossed markets by profit
        crossed_markets.sort_by(|a, b| b.profit.cmp(&a.profit));
        // Return crossed market(s)
        for market in crossed_markets.iter() {
            debug!("{}", market)
        }
        crossed_markets
    }

    pub async fn take_crossed_market<T: Market + ?Sized>(
        &self,
        crossed_market: &CrossedMarketDetails<'_, T>,
        account: &Address,
        ape_weth_balance: U256,
        weth_balance: &U256,
        eth_balance: &U256,
        chi_balance: &U256,
        free_cost: &U256,
    ) -> (Option<TransactionParameters>, U256) {
        debug!("Generating calls for {}", crossed_market);
        // This will be flattened into a vector of calls later
        let mut calls: Vec<Vec<Call>> = vec![];
        // TODO(Move all the blocking work to rayon)

        // TODO(Handle prepare, setting up approvals, etc)
        // Send tokens to first market if needed
        // TODO(Handle error gracefully)
        let to_first_market = crossed_market
            .ask_market
            .to_first_market(&crossed_market.origin_token, &crossed_market.volume)
            .unwrap();
        // Push calls if any
        if let Some(call) = to_first_market {
            calls.push(call)
        }

        // Perform origin to intermediary transit, sending funds to the next contract
        let buy_call = crossed_market
            .ask_market
            .sell_tokens(
                &crossed_market.origin_token,
                &crossed_market.volume,
                &crossed_market.bid_market.market_address(),
            )
            .unwrap();
        calls.push(buy_call);

        // Calculate the amount out of the first market to swap out of the 2nd market
        let inter = crossed_market.ask_market.get_tokens_out(
            &crossed_market.origin_token,
            &crossed_market.intermediary_token,
            &crossed_market.volume,
        );

        // Perform intermediary to origin transit, sending funds back to contract
        let sell_call = crossed_market
            .bid_market
            .sell_tokens(
                &crossed_market.intermediary_token,
                &inter,
                &self.bundle_executor_contract.address(),
            )
            .unwrap();
        calls.push(sell_call);

        // Flatten vector of vector of calls
        let calls: Vec<Call> = calls.into_iter().flatten().collect();

        // Calculate miner payment

        // The percentage is that of the most "unique" edge
        // TODO(Handle case where no percentage is returned (None))
        let miner_payment_percentage = std::cmp::min(
            crossed_market.bid_market.miner_reward_percentage().unwrap(),
            crossed_market.ask_market.miner_reward_percentage().unwrap(),
        );

        let mut ape = false;
        if weth_balance < &crossed_market.volume {
            // Then we'll need a flash loan
            ape = true;
        }

        // TODO()
        let miner_payment = (crossed_market.profit * miner_payment_percentage) / U256::from(100);

        // Check if we need to convert some of the origin token to eth
        let mut pay_with_weth = false;
        if eth_balance < &miner_payment {
            pay_with_weth = true
        }

        // We aren't sandwiching or backrunning, so we don't care about the desired block so much
        let desired_block = constants::ZERO_U256;

        // Build multicall header
        let mch: MulticallHeader = MulticallHeader::new(
            pay_with_weth,
            false,
            miner_payment.as_u128(),
            desired_block.as_u64(),
        );

        // Encode transaction parameters
        let multicall = Multicall::new(mch, calls);
        let params = multicall.encode_parameters();
        let tx;
        tx = utilities::generate_contract_transaction(
            &self.bundle_executor_contract,
            "ostium",
            params,
            account,
            true,
            miner_payment,
        )
        .await;
        (tx, miner_payment)
    }
}

//
//
#[async_trait]
impl BundleGenerator for CrossedMarketArbitrageEngine {
    async fn generate(
        &self,
        markets: &MarketGraph,
        transport: &Web3<WebSocket>,
        account: &Address,
        gas_price: &GasPrice,
        block_number: &U64,
    ) -> Option<Bundle> {
        // TODO(Make this async)
        // These simulations could all run in parallel
        // TODO(Support multiple origin tokens, non weth)
        let weth: Address = address_book::ORIGIN_TOKENS[0].parse().unwrap();
        let weth_contract = Contract::from_json(
            transport.eth(),
            address_book::WETH_ADDRESS.parse().unwrap(),
            include_bytes!("abis/WETH9.json"),
        )
        .unwrap();
        let weth_balance: &U256 = &self
            .bundle_executor_contract
            .query::<U256, _, _, _>(
                "balanceOf",
                weth,
                None,
                Default::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();

        // Get eth balance of executor
        let eth: Address = address_book::ETH_ADDRESS.parse().unwrap();
        let eth_balance: &U256 = &self
            .bundle_executor_contract
            .query::<U256, _, _, _>(
                "balanceOf",
                eth,
                None,
                Default::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();
        let ape_bank: Address = address_book::APE_BANK.parse().unwrap();
        let ape_weth_balance = weth_contract
            .query::<U256, _, _, _>(
                "balanceOf",
                ape_bank,
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();
        let free_cost = &self
            .bundle_executor_contract
            .query::<U256, _, _, _>(
                "freeCost",
                (),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();
        let chi_balance = &self
            .bundle_executor_contract
            .query::<U256, _, _, _>(
                "gasTokenBalance",
                (),
                None,
                Options::default(),
                BlockId::from(BlockNumber::Latest),
            )
            .await
            .unwrap();

        // This stores all the crossed markets found in the graph
        let sorted_crossed_markets = self.evaluate_markets(markets);

        // This stores all the crossed markets and generated transactions
        let mut crossed_market_transaction_futures = vec![];

        // Go through all the crossed markets, construct calls and evaluate by gas usage
        for crossed_market in sorted_crossed_markets.iter() {
            crossed_market_transaction_futures.push(self.take_crossed_market(
                crossed_market,
                account,
                ape_weth_balance,
                weth_balance,
                eth_balance,
                chi_balance,
                free_cost,
            ));
        }
        let crossed_market_transactions: Vec<(Option<TransactionParameters>, U256)> =
            futures::future::join_all(crossed_market_transaction_futures).await;
        let mut crossed_market_results: Vec<(usize, Transaction)> = vec![];
        for (crossed_market_idx, tx_tup) in crossed_market_transactions.into_iter().enumerate() {
            if let Some(tx) = tx_tup.0 {
                let gas_estimate = tx.gas * U256::from(90) / U256::from(100);
                let profit = sorted_crossed_markets[crossed_market_idx].profit;
                crossed_market_results.push((
                    crossed_market_idx,
                    utilities::Transaction {
                        raw_profit: profit,
                        taken_profit: profit - tx_tup.1,
                        delta_coinbase: tx_tup.1,
                        // TODO(Get this from a local simulation before submitting to filter better)
                        estimated_gas: gas_estimate,
                        parameters: tx,
                        signed: None,
                    },
                ))
            }
        }
        if crossed_market_results.is_empty() {
            return None;
        }
        // Sort desc by profit per gas
        crossed_market_results.sort_by(|cm1, cm2| {
            (cm2.1.delta_coinbase / cm2.1.estimated_gas)
                .cmp(&(cm1.1.delta_coinbase / cm1.1.estimated_gas))
        });
        // TODO(tranche these by gas price and chose)
        // A sneaky move to get stuff the simple-arbitrage kids are not might be to grab the 2nd slot
        let final_txns = vec![crossed_market_results[0].1.clone()];
        let bundle = Bundle {
            bundle_hash: None,
            transactions: final_txns,
            block: *block_number,
        };
        if bundle.effective_gas() > gas_price.low {
            info!(
                "Found Arbitrage: {}",
                sorted_crossed_markets[crossed_market_results[0].0]
            );
            return Some(bundle);
        }
        None
    }
}
