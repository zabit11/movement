use alloy::{
	node_bindings::Anvil,
	primitives::{Address, U256},
	providers::{Provider, ProviderBuilder, WalletProvider},
	signers::{
		k256::{elliptic_curve::SecretKey, Secp256k1},
		local::{LocalSigner, PrivateKeySigner},
	},
};
use alloy_network::{Ethereum, EthereumWallet, NetworkWallet};
use alloy_sol_types::sol;
use anyhow::{Error, Result};
use ethereum_bridge::{
	types::AlloyProvider, AtomicBridgeCounterparty, AtomicBridgeInitiator, Config as EthConfig,
	EthClient,
};
use movement_bridge::{Config as MovementConfig, MovementClient};

pub struct TestHarness {
	pub eth_client: Option<EthClient>,
	pub movement_client: Option<MovementClient>,
}

impl TestHarness {
	pub async fn new_only_eth() -> Self {
		let eth_client = EthClient::new(EthConfig::build_for_test())
			.await
			.expect("Failed to create EthClient");
		Self { eth_client: Some(eth_client), movement_client: None }
	}

	pub fn eth_client(&self) -> Result<&EthClient> {
		self.eth_client
			.as_ref()
			.ok_or_else(|| anyhow::Error::msg("EthClient not initialized"))
	}

	pub fn eth_client_mut(&mut self) -> Result<&mut EthClient> {
		self.eth_client
			.as_mut()
			.ok_or_else(|| anyhow::Error::msg("EthClient not initialized"))
	}

	pub fn set_eth_signer(&mut self, signer: SecretKey<Secp256k1>) -> Address {
		let eth_client = self.eth_client_mut().expect("EthClient not initialized");
		let wallet: &mut EthereumWallet = eth_client.rpc_provider_mut().wallet_mut();
		wallet.register_default_signer(LocalSigner::from(signer));
		<EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(wallet)
	}

	pub fn provider(&self) -> &AlloyProvider {
		self.eth_client().expect("Could not fetch eth client").rpc_provider()
	}

	/// The port that Anvil will listen on.
	pub fn rpc_port(&self) -> u16 {
		self.eth_client().expect("Could not fetch eth client").rpc_port()
	}
}