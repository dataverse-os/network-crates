use std::{collections::HashMap, str::FromStr, sync::Arc};

use anyhow::Context;
// use ethers_core::types::{Block, Transaction};
// use ethers_providers::{Http, Middleware, Provider};
// use futures_util::FutureExt;
use int_enum::IntEnum;
// use once_cell::sync::Lazy;
use primitive_types::H256;
use serde::{Deserialize, Serialize};
// use tokio::sync::Mutex;

use crate::event::AnchorProof;

#[cfg(feature = "ceramic-http-client")]
static PROVIDERS: Lazy<Mutex<Providers>> = Lazy::new(|| Mutex::new(Providers::default()));

#[cfg(feature = "ceramic-http-client")]
pub async fn provider(chain: Chain) -> anyhow::Result<ProviderMiddleware> {
	PROVIDERS.lock().await.provider(chain)
}

#[derive(Debug, Clone)]
#[cfg(feature = "ceramic-http-client")]
pub struct Providers {
	pub providers: HashMap<Chain, ProviderMiddleware>,
}

#[cfg(feature = "ceramic-http-client")]
impl Default for Providers {
	fn default() -> Self {
		let mut rpcs: HashMap<Chain, &str> = HashMap::new();
		rpcs.insert(Chain::EthereumMainnet, "https://eth.llamarpc.com");
		rpcs.insert(Chain::EthereumGnosis, "https://rpc.gnosis.gateway.fm");

		let providers_future = Self::new(rpcs).boxed();
		let providers =
			tokio::task::block_in_place(|| futures::executor::block_on(providers_future)).unwrap();

		providers
	}
}

#[cfg(feature = "ceramic-http-client")]
impl Providers {
	pub async fn new(rpcs: HashMap<Chain, &str>) -> anyhow::Result<Self> {
		let mut providers = HashMap::new();
		for (chain, rpc) in rpcs {
			let provider = Provider::<Http>::try_from(rpc)?;
			let chain_id = provider.get_chainid().await?;
			if chain_id.as_u64() != chain.int_value() {
				anyhow::bail!("chain id mismatch for {:?} {}", chain, rpc);
			}
			providers.insert(chain.clone(), ProviderMiddleware(chain, provider.into()));
		}
		Ok(Self { providers })
	}

	fn provider(&self, chain: Chain) -> anyhow::Result<ProviderMiddleware> {
		if let Some(p) = self.providers.get(&chain) {
			return Ok(p.clone());
		}
		anyhow::bail!("no rpc for chain {:?}", chain)
	}
}

#[cfg(feature = "ceramic-http-client")]
static TRANSACTION_STORE: Lazy<Mutex<HashMap<H256, Transaction>>> =
	Lazy::new(|| Mutex::new(Default::default()));

#[cfg(feature = "ceramic-http-client")]
static BLOCK_STORE: Lazy<Mutex<HashMap<H256, Block<H256>>>> =
	Lazy::new(|| Mutex::new(Default::default()));

#[derive(Debug, Clone)]
#[cfg(feature = "ceramic-http-client")]
pub struct ProviderMiddleware(pub Chain, pub Arc<Provider<Http>>);

#[cfg(feature = "ceramic-http-client")]
impl ProviderMiddleware {
	pub async fn get_transaction(&self, tx_hash: H256) -> anyhow::Result<Transaction> {
		let mut store = TRANSACTION_STORE.lock().await;
		if let Some(transaction) = store.get(&tx_hash) {
			return Ok(transaction.clone());
		}
		tracing::info!(
			transaction_hash = tx_hash.to_string(),
			"fetching transaction"
		);
		let tx = match self.1.get_transaction(tx_hash).await? {
			Some(tx) => tx,
			None => {
				tracing::warn!(
					transaction_hash = tx_hash.to_string(),
					"transaction not found with rpc"
				);
				anyhow::bail!("transaction not found: {}", tx_hash)
			}
		};
		store.insert(tx_hash, tx.clone());
		Ok(tx)
	}

	pub async fn get_block(&self, block_hash: H256) -> anyhow::Result<Block<H256>> {
		let mut store = BLOCK_STORE.lock().await;
		if let Some(block) = store.get(&block_hash) {
			return Ok(block.clone());
		}
		tracing::info!(block_hash = block_hash.to_string(), "fetching block");
		let block = match self.1.get_block(block_hash).await? {
			Some(block) => block,
			None => {
				tracing::warn!(
					block_hash = block_hash.to_string(),
					"block not found with rpc"
				);
				anyhow::bail!("block not found block_hash: {}", block_hash)
			}
		};
		store.insert(block_hash, block.clone());
		Ok(block)
	}
}

#[cfg(feature = "ceramic-http-client")]
pub async fn timestamp(proof: AnchorProof) -> anyhow::Result<i64> {
	let provider = provider(proof.chain()?).await?;
	let tx_hash = proof.tx_hash()?;

	let tx = provider.get_transaction(tx_hash).await?;
	let block_hash = tx.block_hash.context("no block hash")?;
	let block = provider.get_block(block_hash).await?;
	Ok(block.timestamp.as_u64() as i64)
}

#[cfg(test)]
#[cfg(feature = "ceramic-http-client")]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_chain_id() {
		let chain = Chain::EthereumMainnet;
		assert_eq!(chain.chain_id(), "eip155:1".to_string());

		let chain = Chain::EthereumGnosis;
		assert_eq!(chain.chain_id(), "eip155:100".to_string());

		let chain = Chain::EthereumGoerli;
		assert_eq!(chain.chain_id(), "eip155:5".to_string());

		let chain = Chain::None;
		assert_eq!(chain.chain_id(), "none".to_string());
	}

	#[tokio::test]
	async fn network() -> anyhow::Result<()> {
		let ceramic = "https://dataverseceramicdaemon.com";
		let http_client = crate::http::Client::init(ceramic)?;
		let chains = http_client.chains().await?;
		assert_eq!(chains.supported_chains, std::vec!["eip155:1"]);
		let chain = chains.supported_chains[0].parse::<Chain>();
		assert!(chain.is_ok());
		let network = chain.unwrap();
		assert_eq!(network, Chain::EthereumMainnet);
		Ok(())
	}

	#[tokio::test]
	async fn test_providers_new() {
		let mut rpcs = HashMap::new();
		rpcs.insert(Chain::EthereumMainnet, "https://eth.llamarpc.com".into());
		rpcs.insert(
			Chain::EthereumGnosis,
			"https://rpc.gnosis.gateway.fm".into(),
		);
		let providers = Providers::new(rpcs).await;
		assert!(providers.is_ok());
	}
}
