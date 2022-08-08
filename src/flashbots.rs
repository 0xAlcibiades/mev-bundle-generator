use std::convert::TryInto;

use anyhow::anyhow;
use async_trait::async_trait;
use log::{debug, error, warn};
use secp256k1::SecretKey;
use surf::http::mime::JSON;
use tiny_keccak::Hasher;
use web3::signing::{Key, SecretKeyRef};
use web3::transports::WebSocket;
use web3::types::{Address, BlockNumber, H160, U256, U64};
use web3::Web3;

use crate::gas::GasPrice;
use crate::markets::MarketGraph;
use crate::{constants, utilities, wallet};

// TODO(Consider a signing thread to reduce overhead on each signature)

/// Return keccak 256 hash of to_hash.
fn keccak256(to_hash: &[u8]) -> [u8; 32] {
    let mut hasher = tiny_keccak::Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(to_hash);
    hasher.finalize(&mut output);
    output
}

fn sign_body(body: &str, private_key: &SecretKey) -> String {
    // Create a web3 reference wrapper for the secp256k1 secret key
    let flashbots_key_rf = SecretKeyRef::new(private_key);
    // EIP-191 Salt
    // TODO: 66 is the len of the message in this case. It should not change, but would be better
    // to compute below in the eth_message.
    let eth_salt = "\x19Ethereum Signed Message:\n66".to_string();
    // Take Kekkak
    // These 0x prefixes are a bane.
    let prefix = "0x";
    let mut digest = prefix.to_owned();
    digest.push_str(&hex::encode(keccak256(body.as_bytes()).to_owned()));
    let digest = keccak256(&[eth_salt.as_bytes(), &digest.as_bytes()].concat()).to_owned();
    // Sign with key
    let signature = flashbots_key_rf.sign(&digest, None).unwrap();
    // Get recovery bits
    let v = signature
        .v
        .try_into()
        .expect("signature recovery in electrum notation always fits in a u8");
    // Build signature utf8 vector
    let signature_bytes = {
        let mut bytes = Vec::with_capacity(65);
        bytes.extend_from_slice(signature.r.as_bytes());
        bytes.extend_from_slice(signature.s.as_bytes());
        bytes.push(v);
        bytes
    };
    // Return hex encoded string from utf8 signature_bytes vector with 0x prefix
    let mut ret = prefix.to_owned();
    ret.push_str(&hex::encode(signature_bytes));
    ret
}

#[derive(Debug, Copy, Clone)]
pub enum OperationMode {
    Send,
    Simulate,
}

#[derive(Debug, Clone)]
pub struct Bundle {
    // TODO(Set this after a bundle is submitted)
    pub bundle_hash: Option<H160>,
    pub transactions: Vec<utilities::Transaction>,
    pub block: U64,
}

impl Bundle {
    pub fn taken_profit(&self) -> U256 {
        let mut taken_profit = constants::ZERO_U256;
        for transaction in &self.transactions {
            taken_profit += transaction.taken_profit;
        }
        taken_profit
    }

    /// Return the bundle score
    pub fn score(&self) -> U256 {
        // TODO(Implement detection of duplicate transaction in mempool)
        self.effective_gas()
    }

    pub fn miner_payment(&self) -> U256 {
        let mut delta_coinbase = constants::ZERO_U256;
        let mut gas_payment = constants::ZERO_U256;
        for transaction in &self.transactions {
            delta_coinbase += transaction.delta_coinbase;
            let gas_paid = match transaction.parameters.gas_price {
                Some(gas) => gas,
                None => constants::ZERO_U256,
            };
            gas_payment += transaction.estimated_gas * gas_paid;
        }
        delta_coinbase + gas_payment
    }

    /// Return the effective gas price for the bundle
    pub fn effective_gas(&self) -> U256 {
        let mut gas_used_estimate = constants::ZERO_U256;
        for transaction in &self.transactions {
            gas_used_estimate += transaction.estimated_gas;
        }
        self.miner_payment() / gas_used_estimate
    }

    /// Call a simulation or send to the flashbots relay
    pub async fn submit(
        &mut self,
        web3_transport: &Web3<WebSocket>,
        operation_mode: &OperationMode,
        executor: &wallet::LocalWallet,
        flashbots_signer: &wallet::LocalWallet,
        client: &surf::Client,
        relay: &str,
    ) -> anyhow::Result<String> {
        // TODO(Consider ethers library with this as a provider)
        // TODO(Sign transaction only if it isn't already signed)
        executor
            .sign_transactions(web3_transport, &mut self.transactions, None)
            .await;
        let mut raw_transactions = vec![];
        for tx in &self.transactions {
            if let Some(tx) = tx.signed.clone() {
                raw_transactions.push(tx.raw_transaction)
            }
        }
        let send = match operation_mode {
            OperationMode::Send => true,
            OperationMode::Simulate => false,
        };
        // TODO(Support for a relay address as an argument)
        let fb_req = if std::ops::Not::not(send) {
            debug!("Simulate Bundle");
            // Generate request JSON
            // TODO(Add support for optional parameters)
            serde_json::json!({
   "jsonrpc": "2.0",
   "method": "eth_callBundle",
   "params": [{"txs": raw_transactions, "blockNumber": BlockNumber::from(self.block + 1_u64), "stateBlockNumber": BlockNumber::from(self.block)}],
   "id": 1}).to_string()
        } else {
            debug!("Send Bundle");
            // Generate request JSON
            // TODO(Add support for optional parameters)
            serde_json::json!({
       "jsonrpc": "2.0",
       "method": "eth_sendBundle",
       "params": [{"txs": raw_transactions, "blockNumber": BlockNumber::from(self.block + 1_u64)}],
       "id": 1})
            .to_string()
        };
        let fb_req_sig_header = format!(
            "{}:{}",
            &flashbots_signer.address(),
            sign_body(&fb_req.to_string(), &flashbots_signer.private_key)
        );
        // TODO(Add goerli testnet support)
        debug!("X-Flashbots-Signature: {}", fb_req_sig_header);
        debug!("Request JSON: {}", &fb_req);
        let mut req: surf::Request = surf::post(relay).build();
        req.set_header("X-Flashbots-Signature", &fb_req_sig_header);
        req.set_content_type(JSON);
        req.set_body(fb_req);
        let res = client.send(req);
        let mut res = res.await.unwrap();
        let body = res.body_string().await.unwrap();
        if res.status() != 200 {
            match operation_mode {
                OperationMode::Simulate => warn!("{}", body),
                OperationMode::Send => error!("{}", body),
            }
            return Err(anyhow!("{}", body));
        }
        debug!("{}", body);
        Ok(body)
    }
}

// TODO(Bundle generator trait)
#[async_trait]
pub trait BundleGenerator {
    async fn generate(
        &self,
        markets: &MarketGraph,
        transport: &Web3<WebSocket>,
        account: &Address,
        gas_price: &GasPrice,
        block_number: &U64,
    ) -> Option<Bundle>;
}
