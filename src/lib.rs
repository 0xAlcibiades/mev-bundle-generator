use std::env;

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use tokio::join;
use web3::api::SubscriptionStream;
use web3::futures::StreamExt;
use web3::transports::WebSocket;
use web3::types::{Block, BlockHeader, BlockId, BlockNumber, FilterBuilder, Log, SyncState, H256};
use web3::Web3;

use crate::flashbots::{Bundle, BundleGenerator, OperationMode};
use crate::markets::MarketGraph;
use crate::wallet::LocalWallet;

mod address_book;
mod arbitrage;
mod compound;
mod constants;
mod evm;
mod flashbots;
mod gas;
mod markets;
mod sushiswap;
mod uniswap;
mod utilities;
mod wallet;
mod weth_token;

// TODO(It would be ideal to dynamically update blocklists for addresses causing reverts)

pub struct Config {
    pub executor_pk: String,
    pub flashbots_pk: String,
    pub ws_rpc: String,
    pub operation_mode: OperationMode,
    pub simulation_relay: String,
}

impl Config {
    pub fn new() -> Result<Config> {
        let ws_rpc = env::var("WEB_SOCKET").context("Set the WEB_SOCKET environment variable.")?;
        let executor_pk = env::var("PRIVATE_KEY")
            .context("Set the PRIVATE_KEY environment variable.")?[2..]
            .to_string();
        let flashbots_pk = env::var("FLASHBOTS_KEY")
            .context("Set the FLASHBOTS_KEY environment variable.")?[2..]
            .to_string();
        let simulation_relay = env::var("SIMULATION_RELAY")
            .context("Set the SIMULATION_RELAY environment variable.")?;
        let operation_mode = env::var("SIMULATE_ONLY");
        let operation_mode = match operation_mode {
            Err(_) => OperationMode::Send,
            Ok(_) => {
                info!("Running in simulation only mode.");
                OperationMode::Simulate
            }
        };
        Ok(Config {
            executor_pk,
            flashbots_pk,
            ws_rpc,
            operation_mode,
            simulation_relay,
        })
    }
}

// Info we care about for each block
#[derive(Clone, Debug)]
struct BlockInfo {
    pub block: Option<Block<H256>>,
    pub logs: Option<Vec<Log>>,
    pub sync: SyncState,
    pub gas_price: gas::GasPrice,
}

impl BlockInfo {
    pub async fn new(rpc: &Web3<WebSocket>) -> Result<BlockInfo> {
        // Really this should just return early and empty when syncing

        let mut block: Option<Block<H256>> = None;
        let mut logs: Option<Vec<Log>> = None;
        let (block_w, logs_w, sync, gas_price) = join![
            rpc.eth().block(BlockId::from(BlockNumber::Latest)),
            rpc.eth().logs(
                FilterBuilder::default()
                    .from_block(BlockNumber::Latest)
                    .to_block(BlockNumber::Latest)
                    .build()
            ),
            rpc.eth().syncing(),
            // TODO(Should this even be loaded in the sync state?)
            gas::GasPrice::new(&rpc)
        ];
        let sync = sync?;
        if let SyncState::NotSyncing = sync {
            logs = Some(logs_w?);
            block = block_w?;
        }
        Ok(BlockInfo {
            block,
            logs,
            sync,
            gas_price,
        })
    }
}

struct RunData {
    pub executor: LocalWallet,
    pub flashbots_signer: LocalWallet,
    pub rpc: Web3<WebSocket>,
    pub http_client: surf::Client,
    pub operation_mode: OperationMode,
    pub simulation_relay: String,
}

impl RunData {
    pub async fn new(config: &Config) -> Result<RunData> {
        let executor = LocalWallet::new(&config.executor_pk)
            .context("Failed to parse ethereum private key.")?;
        let flashbots_signer = LocalWallet::new(&config.flashbots_pk)
            .context("Failed to parse flashbots bundle signing key.")?;
        let rpc: Web3<WebSocket> = web3::Web3::new(
            web3::transports::WebSocket::new(&config.ws_rpc)
                .await
                .context("Failed to connect to ethereum RPC websocket.")?,
        );
        let http_client: surf::Client = surf::Client::new();
        // TODO(Enable and configure these based on a config file)
        Ok(RunData {
            executor,
            flashbots_signer,
            rpc,
            http_client,
            operation_mode: config.operation_mode,
            simulation_relay: config.simulation_relay.to_string(),
        })
    }
}

// TODO(Break down this function further)
async fn search(
    markets: &MarketGraph,
    bundle_generators: &mut Vec<Box<dyn BundleGenerator>>,
    run_data: &mut RunData,
    block_info: &BlockInfo,
) -> Result<()> {
    let block_number = block_info.block.as_ref().unwrap().number.unwrap();
    info!(
        "Searching for opportunities in block #{}.",
        block_number + 1
    );
    let mut bundle_futures = vec![];
    let mut bundles: Vec<Bundle> = vec![];
    // TODO(Collect a vector of futures here)
    for bundle_generator in bundle_generators.iter_mut() {
        bundle_futures.push(bundle_generator.generate(
            &markets,
            &run_data.rpc,
            &run_data.executor.public_key,
            &block_info.gas_price,
            &block_number,
        ));
    }
    // TODO(Fix this)
    let bundle_options = futures::future::join_all(bundle_futures).await;
    for bundle in bundle_options.into_iter().flatten() {
        bundles.push(bundle)
    }
    if bundles.is_empty() {
        info!(
            "No opportunities discovered in block #{}.",
            &block_number + 1
        );
    } else {
        // TODO(Consider that bundles with an effective score below gas_pricing.medium are often discarded)
        // The relay will just throw them away, they should be funneled to PGA
        // TODO(Handle multiple bundles, mainly around correcting nonce values)
        let mut best_bundle: Bundle = bundles[0].clone();
        let mut best_score = constants::ZERO_U256;
        // TODO(Process simulations async, against local mev-geth)
        for mut bundle in bundles {
            // TODO(Drop bundle on error)
            // TODO(Update gas usage from simulation for effective scoring)
            let simulation = bundle
                .submit(
                    &run_data.rpc,
                    &flashbots::OperationMode::Simulate,
                    &run_data.executor,
                    &run_data.flashbots_signer,
                    &run_data.http_client,
                    &run_data.simulation_relay,
                )
                .await;
            match simulation {
                Ok(_) => (),
                Err(_) => continue,
            }
            // I.E. if error, continue (logging error)
            let score = bundle.score();
            if score > best_score {
                best_score = score;
                best_bundle = bundle;
            }
        }
        // TODO(Replace with display)
        match run_data.operation_mode {
            OperationMode::Simulate => (),
            OperationMode::Send => {
                let mut fb_bun = best_bundle.clone();
                let fb_fut = fb_bun.submit(
                    &run_data.rpc,
                    &flashbots::OperationMode::Send,
                    &run_data.executor,
                    &run_data.flashbots_signer,
                    &run_data.http_client,
                    "https://relay.flashbots.net/",
                );
                let em_fut = best_bundle.submit(
                    &run_data.rpc,
                    &flashbots::OperationMode::Send,
                    &run_data.executor,
                    &run_data.flashbots_signer,
                    &run_data.http_client,
                    "https://mev-relay.ethermine.org/",
                );
                let (em, fb) = join!(em_fut, fb_fut);
                match fb {
                    Ok(_) => {
                        let bundle_gas = utilities::to_gwei(&best_bundle.effective_gas());
                        info!(
                            "Bundle submitted to flashbots with effective gas rate of {} gwei a profit sent to miner of Ξ{}, and a taken profit of Ξ{}.",
                            bundle_gas, utilities::to_ether(&best_bundle.miner_payment()), utilities::to_ether(&best_bundle.taken_profit())
                        );
                        if best_bundle.effective_gas() < block_info.gas_price.low {
                            warn!(
                                "Bundle discovered and submitted unlikely to be included due to gas \
                          price being at the bottom of the block."
                            )
                        };
                    }
                    Err(_) => error!("Bundle submission to flashbots failed."),
                }
                match em {
                    Ok(_) => {
                        let bundle_gas = utilities::to_gwei(&best_bundle.effective_gas());
                        info!(
                        "Bundle submitted to ethermine with effective gas rate of {} gwei a profit sent to miner of Ξ{}, and a taken profit of Ξ{}.",
                        bundle_gas, utilities::to_ether(&best_bundle.miner_payment()), utilities::to_ether(&best_bundle.taken_profit())
                    );
                        if best_bundle.effective_gas() < block_info.gas_price.low {
                            warn!(
                            "Bundle discovered and submitted unlikely to be included due to gas \
                      price being at the bottom of the block."
                        )
                        };
                    }
                    Err(_) => error!("Bundle submission to ethermine failed."),
                }
            }
        };
    }
    Ok(())
}

async fn loop_blocks(run_data: &mut RunData) -> Result<()> {
    debug!("Setting up market graph.");
    let mut market_graph = MarketGraph::new(&run_data.rpc).await;
    let mut bundle_generators: Vec<Box<dyn BundleGenerator>> = vec![];
    bundle_generators.push(Box::new(
        arbitrage::CrossedMarketArbitrageEngine::new(&run_data.rpc).await,
    ));
    let mut block_subscription: SubscriptionStream<WebSocket, BlockHeader> =
        run_data.rpc.eth_subscribe().subscribe_new_heads().await?;
    info!("Waiting for first block header from Ethereum client RPC.");
    let mut full_update = true;
    'blocks: while block_subscription.next().await.is_some() {
        // TODO(Track last block and trigger full update if there is a discontinuity)
        // It seems like the syncing state is not dependable
        // Let's make sure we are at the chainhead
        let block_info = BlockInfo::new(&run_data.rpc).await?;
        match block_info.sync {
            SyncState::NotSyncing => {
                let block = block_info.clone().block.unwrap();
                info!(
                    "Finalized block #{}, Mining Started Timestamp: {}.",
                    block.number.unwrap(),
                    chrono::NaiveDateTime::from_timestamp(i64::from(block.timestamp.as_u32()), 0)
                )
            }
            SyncState::Syncing(syncinfo) => {
                info!(
                    "Ethereum client syncing block #{}/{}.",
                    syncinfo.current_block, syncinfo.highest_block
                );
                // This will trigger a full update of state for the next block
                full_update = true;
                continue 'blocks;
            }
        }
        // Search for and submit and opportunities found within the block.
        if full_update {
            // Update all state data
            market_graph.update_all().await;
        } else {
            // Update only the state delta
            market_graph
                .update_delta(&block_info.logs.as_ref().unwrap())
                .await;
        }
        search(&market_graph, &mut bundle_generators, run_data, &block_info).await?;
        // The next block will only need to update state deltas
        full_update = false;
    }
    Ok(())
}

/// Display startup message
fn print_startup(run_data: &RunData) {
    info!("Searcher wallet address: {}.", run_data.executor.address());
    info!(
        "Flashbots relay signing wallet address: {}.",
        run_data.flashbots_signer.address()
    );
}

// This is wrapped up in a thread pool for call by the binary.
#[tokio::main]
pub async fn run(config: &Config) -> Result<()> {
    // Setup runtime data from configuration
    let mut run_data = RunData::new(&config).await?;
    // Tell the user some stuff confirming configuration
    print_startup(&run_data);
    // An "infinite" loop over incoming blocks.
    loop_blocks(&mut run_data).await
}
