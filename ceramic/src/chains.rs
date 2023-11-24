pub enum Chain {
    EthereumMainnet,
    EthereumGnosis,
}

impl Chain {
    pub fn chain_id(&self) -> String {
        match self {
            Chain::EthereumMainnet => "eip155:1".to_string(),
            Chain::EthereumGnosis => "eip155:100".to_string(),
        }
    }
}

pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    pub fn pubsub_topic(&self) -> String {
        match self {
            Network::Mainnet => "/ceramic/mainnet".to_string(),
            Network::Testnet => "/ceramic/testnet-clay".to_string(),
        }
    }

    pub fn chain(&self) -> Chain {
        match self {
            Network::Mainnet => Chain::EthereumMainnet,
            Network::Testnet => Chain::EthereumGnosis,
        }
    }
}
