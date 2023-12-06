use std::{collections::HashMap, str::FromStr};

use ethers_providers::{Http, Middleware, Provider};
use int_enum::IntEnum;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[repr(u64)]
#[derive(Debug, Clone, Copy, IntEnum, PartialEq, Eq, Hash)]
pub enum Chain {
    EthereumMainnet = 1,
    EthereumGnosis = 100,
    EthereumGoerli = 5,
    None = 0,
}

impl Chain {
    pub fn chain_id(&self) -> String {
        match self {
            Chain::None => "none".to_string(),
            _ => format!("eip155:{}", self.int_value()),
        }
    }

    pub fn network(&self) -> Network {
        match self {
            Chain::EthereumMainnet => Network::Mainnet,
            Chain::EthereumGnosis => Network::TestnetClay,
            Chain::EthereumGoerli => Network::DevUnstable,
            Chain::None => Network::InMemory,
        }
    }
}

impl FromStr for Chain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eip155:1" => Ok(Chain::EthereumMainnet),
            "eip155:100" => Ok(Chain::EthereumGnosis),
            "eip155:5" => Ok(Chain::EthereumGoerli),
            _ => anyhow::bail!("invalid chain"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Network {
    /// Production network
    Mainnet,
    /// Test network
    TestnetClay,
    /// Developement network
    DevUnstable,
    /// Local network with unique id
    Local(u32),
    /// Singleton network in memory
    InMemory,
}

impl From<ceramic_core::Network> for Network {
    fn from(network: ceramic_core::Network) -> Self {
        match network {
            ceramic_core::Network::Mainnet => Network::Mainnet,
            ceramic_core::Network::TestnetClay => Network::TestnetClay,
            ceramic_core::Network::DevUnstable => Network::DevUnstable,
            ceramic_core::Network::Local(i) => Network::Local(i),
            ceramic_core::Network::InMemory => Network::InMemory,
        }
    }
}

impl Network {
    pub fn public(&self) -> bool {
        match self {
            Network::Mainnet => true,
            Network::TestnetClay => true,
            Network::DevUnstable => true,
            Network::Local(_) => false,
            Network::InMemory => false,
        }
    }

    pub fn kubo_topic(&self) -> String {
        multibase::encode(multibase::Base::Base64Url, self.pubsub_topic())
    }

    pub fn pubsub_topic(&self) -> String {
        match self {
            Network::Mainnet => "/ceramic/mainnet".to_string(),
            Network::TestnetClay => "/ceramic/testnet-clay".to_string(),
            Network::DevUnstable => "/ceramic/dev-unstable".to_string(),
            Network::Local(i) => format!("/ceramic/local-{}", i),
            Network::InMemory => "/ceramic/inmemory".to_owned(),
        }
    }

    pub fn chain(&self) -> Chain {
        match self {
            Network::Mainnet => Chain::EthereumMainnet,
            Network::TestnetClay => Chain::EthereumGnosis,
            Network::DevUnstable => Chain::EthereumGoerli,
            Network::Local(_) => Chain::None,
            Network::InMemory => Chain::None,
        }
    }
}

static PROVIDERS: Lazy<Mutex<Providers>> = Lazy::new(|| Mutex::new(Providers::default()));

pub async fn provider(chain: Chain) -> anyhow::Result<Provider<Http>> {
    PROVIDERS.lock().await.provider(chain)
}

#[derive(Debug, Clone)]
pub struct Providers {
    pub rpcs: HashMap<Chain, String>,
}

impl Default for Providers {
    fn default() -> Self {
        let mut rpcs = HashMap::new();
        rpcs.insert(Chain::EthereumMainnet, "https://eth.llamarpc.com".into());
        rpcs.insert(
            Chain::EthereumGnosis,
            "https://rpc.gnosis.gateway.fm".into(),
        );
        Self { rpcs }
    }
}

impl Providers {
    pub async fn new(rpcs: HashMap<Chain, String>) -> anyhow::Result<Self> {
        for (chain, rpc) in &rpcs {
            let provider = Provider::<Http>::try_from(rpc)?;
            let chain_id = provider.get_chainid().await?;
            if chain_id.as_u64() != chain.int_value() {
                anyhow::bail!("chain id mismatch for {:?} {}", chain, rpc);
            }
        }
        Ok(Self { rpcs })
    }

    fn provider(&self, chain: Chain) -> anyhow::Result<Provider<Http>> {
        if let Some(rpc) = self.rpcs.get(&chain) {
            let provider = Provider::<Http>::try_from(rpc)?;
            return Ok(provider);
        }
        anyhow::bail!("no rpc for chain {:?}", chain)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

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
        assert_eq!(chains.supported_chains, vec!["eip155:1"]);
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
