use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Chain {
    EthereumMainnet,
    EthereumGnosis,
    EthereumGoerli,
    None,
}

impl Chain {
    pub fn chain_id(&self) -> String {
        match self {
            Chain::EthereumMainnet => "eip155:1".to_string(),
            Chain::EthereumGnosis => "eip155:100".to_string(),
            Chain::EthereumGoerli => "eip155:5".to_string(),
            Chain::None => "none".to_string(),
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

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[tokio::test]
    async fn network() -> anyhow::Result<()> {
        let ceramic = "https://dataverseceramicdaemon.com";
        let client = crate::http::Client::init(ceramic)?;
        let chains = client.ceramic.chains().await?;
        assert_eq!(chains.supported_chains, vec!["eip155:1"]);
        let chain = chains.supported_chains[0].parse::<Chain>();
        assert!(chain.is_ok());
        let network = chain.unwrap();
        assert_eq!(network, Chain::EthereumMainnet);
        Ok(())
    }
}
