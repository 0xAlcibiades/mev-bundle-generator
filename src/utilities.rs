use std::fmt::Debug;

use bigdecimal::BigDecimal;
use log::warn;
use num_bigint::{BigInt, BigUint};
use secp256k1::SecretKey;
use web3::contract::tokens::Tokenize;
use web3::contract::{Contract, Options};
use web3::transports::WebSocket;
use web3::types::{Address, BlockId, SignedTransaction, TransactionParameters, U256, U64};
use web3::Web3;

// TODO(Add a quantity struct with a U256 base and all of these conversions built in)
use crate::constants;

// It would be ideal to write a 256 bit fixed point math library here using u256 and get
// rid of bigdecimal, bigint, biguint.

/// Convert a U256 to a BigInt
pub fn u256_bigint(num: web3::types::U256) -> BigInt {
    let mut num_bytes = [b'0'; 32];
    num.to_little_endian(&mut num_bytes);
    BigInt::from(BigUint::from_bytes_le(&num_bytes))
}

/// Convert a BigInt into a BigDecimal
pub fn bigint_bigdecimal(num: BigInt) -> BigDecimal {
    BigDecimal::from(num)
}

/// Convert a U256 of wei to an ether BigDecimal
pub fn to_ether(wei: &web3::types::U256) -> BigDecimal {
    let ether: BigDecimal = BigDecimal::from(10_u64.pow(18));
    let wei = bigint_bigdecimal(u256_bigint(*wei));
    wei / ether
}

/// Convert a U256 of wei to a
pub fn to_gwei(wei: &web3::types::U256) -> BigDecimal {
    // TODO(Consider a U64 here)
    let ether: BigDecimal = BigDecimal::from(10_u64.pow(9));
    let wei = bigint_bigdecimal(u256_bigint(*wei));
    wei / ether
}

/// Get the present nonce for an address
pub async fn nonce(public_key: &Address, transport: &Web3<WebSocket>) -> U256 {
    transport
        .eth()
        .transaction_count(*public_key, None)
        .await
        .unwrap()
}

pub async fn sushi_balance(
    sushi: &Contract<WebSocket>,
    public_key: Address,
    block_number: U64,
) -> U256 {
    sushi
        .query::<U256, _, _, _>(
            "balanceOf",
            public_key,
            None,
            Options::default(),
            BlockId::from(block_number),
        )
        .await
        .unwrap()
}

/// Estimates gas, generates metadata and returns metadata and transaction parameters
pub async fn generate_contract_transaction(
    contract: &Contract<WebSocket>,
    func: &str,
    params: impl Tokenize + Clone + Debug,
    account: &Address,
    estimate_gas: bool,
    miner_payment: U256,
) -> Option<TransactionParameters> {
    // TODO(Maybe support this as an argument for non flashbots integration)
    let mut gas_price = Some(constants::ZERO_U256);
    let estimated_gas = if estimate_gas {
        let estimated_gas = contract
            .estimate_gas(
                func,
                params.clone(),
                *account,
                Options {
                    gas: None,
                    gas_price,
                    value: None,
                    nonce: None,
                    condition: None,
                    transaction_type: None,
                    access_list: None,
                },
            )
            .await;
        // Let's return if estimate gas fails
        // TODO(Hex encode these debugs for sanity)
        let estimated_gas = match estimated_gas {
            Ok(estimated_gas) => estimated_gas,
            Err(error) => {
                warn!(
                    "Call: {} on: {} with params: {:?}",
                    func,
                    contract.address(),
                    params.clone()
                );
                warn!("EVM error: {:?}", error);
                return None;
            }
        };
        estimated_gas
    } else {
        // This is mainly for miner bribe txs
        U256::from(200001_u64)
    };
    gas_price = Some(miner_payment / (estimated_gas / 90 * 100));
    let data: web3::types::Bytes = contract
        .abi()
        .function(func)
        .unwrap()
        .encode_input(&*params.into_tokens())
        .unwrap()
        .into();
    // TODO(Support the optional input for these params)
    Some(TransactionParameters {
        nonce: None,
        to: Option::from(contract.address()),
        gas: estimated_gas,
        gas_price,
        value: Default::default(),
        data,
        chain_id: Some(1_u64),
        transaction_type: None,
        access_list: None,
    })
}

/// Aggregate data about a transaction for profit and loss.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub raw_profit: U256,
    pub taken_profit: U256,
    pub delta_coinbase: U256,
    // Because estimated_gas is not the same as parameters.gas_limit
    pub estimated_gas: U256,
    pub parameters: TransactionParameters,
    pub signed: Option<SignedTransaction>,
}

impl Transaction {
    /// Signs and returns transaction.
    pub async fn sign(&mut self, transport: &Web3<WebSocket>, private_key: &SecretKey) {
        let signed = transport
            .accounts()
            .sign_transaction(self.parameters.clone(), private_key)
            .await
            .unwrap();
        self.signed = Some(signed);
    }
}

// Metadata for ensuring rewards from txs which must be called by an EOA
pub(crate) struct RewardsMeta {
    pub reward_token: Address,
    pub reward_amount: U256,
}
