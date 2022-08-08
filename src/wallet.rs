use anyhow::{Context, Result};
use hex::ToHex;
use secp256k1::SecretKey;
use web3::signing::{Key, SecretKeyRef};
use web3::transports::WebSocket;
use web3::types::{Address, U256};
use web3::Web3;

use crate::utilities;

/// A local wallet keypair
#[derive(Debug, Copy, Clone)]
pub struct LocalWallet {
    pub public_key: Address,
    // Yas, yas, the ironiz
    pub private_key: SecretKey,
}

impl LocalWallet {
    pub fn new(private_key: &str) -> Result<LocalWallet> {
        let private_key =
            SecretKey::from_slice(&hex::decode(private_key).context("Failed to decode key.")?)
                .context("Failed to load private key.")?;
        let public_key = SecretKeyRef::new(&private_key).address();
        Ok(LocalWallet {
            public_key,
            private_key,
        })
    }

    pub fn address(self) -> String {
        // TODO(Figure out how to use the serialization of public_key directly)
        format!("0x{}", self.public_key.0.encode_hex::<String>())
    }

    pub async fn sign_transactions(
        self,
        transport: &Web3<WebSocket>,
        transactions: &mut Vec<utilities::Transaction>,
        start_nonce: Option<U256>,
    ) {
        // TODO(Consider a sort method for deterministic nonces)
        let mut start_nonce = match start_nonce {
            Some(nonce) => nonce,
            None => utilities::nonce(&self.public_key, transport).await,
        };
        for transaction in transactions {
            transaction.parameters.nonce = Some(start_nonce);
            transaction.sign(transport, &self.private_key).await;
            start_nonce += U256::from(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_wallet() {
        let wallet = LocalWallet::new(
            &"a8cc72b6a413343939c859d7f48f665812a293679c2eb6fcb3ab861d84c07cae".to_string(),
        )
        .unwrap();
        // TODO(Add checksum for addresses)
        // assert_eq!(wallet.address, "0xb553a515F6370FA73819cb5fcf4C5ce8826f6829".to_string())
        assert_eq!(
            wallet.address(),
            "0xb553a515f6370fa73819cb5fcf4c5ce8826f6829".to_string()
        )
    }

    #[test]
    #[should_panic]
    fn construct_wallet_invalid() {
        let _wallet = LocalWallet::new(
            &"0xa8cc72b6a413343939c859d7f48f665812a293679c2eb6fcb3ab861d84c07cae".to_string(),
        )
        .unwrap();
    }
}
