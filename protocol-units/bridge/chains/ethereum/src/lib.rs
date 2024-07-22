use alloy::pubsub::PubSubFrontend;
use alloy::signers::local::PrivateKeySigner;
use alloy_network::{Ethereum, EthereumWallet};
use alloy_primitives::private::serde::{Deserialize, Serialize};
use alloy_primitives::{FixedBytes, U256};
use alloy_provider::{
	fillers::{ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, WalletFiller},
	Provider, ProviderBuilder, RootProvider,
};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use alloy_sol_types::sol;
use alloy_transport::BoxTransport;
use alloy_transport_ws::WsConnect;
use anyhow::Context;
use bridge_shared::types::{
	Amount, BridgeTransferDetails, BridgeTransferId, CounterpartyCompletedDetails, HashLock,
	HashLockPreImage, InitiatorAddress, RecipientAddress, TimeLock,
};
use bridge_shared::{
	bridge_contracts::{
		BridgeContractCounterpartyError, BridgeContractInitiator, BridgeContractInitiatorError,
		BridgeContractInitiatorResult,
	},
	bridge_monitoring::{BridgeContractCounterpartyEvent, BridgeContractInitiatorEvent},
	types::LockDetails,
};
use keccak_hash::keccak;
use mcr_settlement_client::send_eth_transaction::{
	send_transaction, InsufficentFunds, SendTransactionErrorRule, UnderPriced, VerifyRule,
};
use std::fmt::Debug;
use thiserror::Error;
use utils::EthAddress;

pub mod utils;

const INITIATOR_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const RECIPIENT_ADDRESS: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const DEFAULT_GAS_LIMIT: u64 = 10_000_000_000;
const MAX_RETRIES: u32 = 5;

type EthHash = [u8; 32];

pub type SCIResult<A, H> = Result<BridgeContractInitiatorEvent<A, H>, BridgeContractInitiatorError>;
pub type SCCResult<A, H> =
	Result<BridgeContractCounterpartyEvent<A, H>, BridgeContractCounterpartyError>;

///Configuration for the Ethereum Bridge Client
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
	pub rpc_url: Option<String>,
	pub ws_url: Option<String>,
	pub chain_id: String,
	pub signer_private_key: String,
	pub initiator_address: EthAddress,
	pub recipient_address: String,
	pub gas_limit: u64,
	pub num_tx_send_retries: u32,
}

impl Default for Config {
	fn default() -> Self {
		Config {
			rpc_url: Some("http://localhost:8545".to_string()),
			ws_url: Some("ws://localhost:8545".to_string()),
			chain_id: "31337".to_string(),
			signer_private_key: Self::default_for_private_key(),
			initiator_address: EthAddress::from(INITIATOR_ADDRESS.to_string()),
			recipient_address: RECIPIENT_ADDRESS.to_string(),
			gas_limit: DEFAULT_GAS_LIMIT,
			num_tx_send_retries: MAX_RETRIES,
		}
	}
}

impl Config {
	fn default_for_private_key() -> String {
		let random_wallet = PrivateKeySigner::random();
		random_wallet.to_bytes().to_string()
	}
}

// Codegen from the abi
sol!(
	#[allow(missing_docs)]
	#[sol(rpc)]
	AtomicBridgeInitiator,
	"abis/AtomicBridgeInitiator.json"
);

type AlloyProvider = FillProvider<
	JoinFill<
		JoinFill<
			JoinFill<JoinFill<alloy::providers::Identity, GasFiller>, NonceFiller>,
			ChainIdFiller,
		>,
		WalletFiller<EthereumWallet>,
	>,
	RootProvider<BoxTransport>,
	BoxTransport,
	Ethereum,
>;

#[derive(RlpDecodable, RlpEncodable)]
struct EthBridgeTransferDetails {
	pub amount: U256,
	pub originator: EthAddress,
	pub recipient: [u8; 32],
	pub hash_lock: [u8; 32],
	pub time_lock: U256,
	pub state: u8, // Assuming the enum is u8 for now..
}

pub struct EthClient<P> {
	rpc_provider: P,
	chain_id: String,
	ws_provider: RootProvider<PubSubFrontend>,
	initiator_address: EthAddress,
	send_transaction_error_rules: Vec<Box<dyn VerifyRule>>,
	gas_limit: u64,
	num_tx_send_retries: u32,
}

impl EthClient<AlloyProvider> {
	pub async fn build_with_config(
		config: Config,
		counterparty_address: &str,
	) -> Result<Self, anyhow::Error> {
		let signer = config.signer_private_key.parse::<PrivateKeySigner>()?;
		let rpc_url = config.rpc_url.context("rpc_url not set")?;
		let ws_url = config.ws_url.context("ws_url not set")?;
		let rpc_provider = ProviderBuilder::new()
			.with_recommended_fillers()
			.wallet(EthereumWallet::from(signer.clone()))
			.on_builtin(&rpc_url)
			.await?;
		let ws = WsConnect::new(ws_url);
		let ws_provider = ProviderBuilder::new().on_ws(ws).await?;

		EthClient::build_with_provider(utils::ProviderArgs {
			rpc_provider,
			ws_provider,
			initiator_address: config.initiator_address,
			counterparty_address: counterparty_address.parse()?,
			gas_limit: config.gas_limit,
			num_tx_send_retries: config.num_tx_send_retries,
			chain_id: config.chain_id,
		})
		.await
	}

	async fn build_with_provider(args: utils::ProviderArgs) -> Result<Self, anyhow::Error> {
		let rule1: Box<dyn VerifyRule> = Box::new(SendTransactionErrorRule::<UnderPriced>::new());
		let rule2: Box<dyn VerifyRule> =
			Box::new(SendTransactionErrorRule::<InsufficentFunds>::new());
		let send_transaction_error_rules = vec![rule1, rule2];
		Ok(EthClient {
			rpc_provider: args.rpc_provider,
			chain_id: args.chain_id,
			ws_provider: args.ws_provider,
			initiator_address: args.initiator_address,
			gas_limit: args.gas_limit,
			num_tx_send_retries: args.num_tx_send_retries,
			send_transaction_error_rules,
		})
	}
}

impl<P> Clone for EthClient<P> {
	fn clone(&self) -> Self {
		todo!()
	}
}

#[async_trait::async_trait]
impl<P> BridgeContractInitiator for EthClient<P>
where
	P: Provider + Clone + Send + Sync + Unpin,
{
	type Address = EthAddress;
	type Hash = EthHash;

	async fn initiate_bridge_transfer(
		&mut self,
		_initiator_address: InitiatorAddress<Self::Address>,
		recipient_address: RecipientAddress<Vec<u8>>,
		hash_lock: HashLock<Self::Hash>,
		time_lock: TimeLock,
		amount: Amount,
	) -> BridgeContractInitiatorResult<()> {
		let contract = AtomicBridgeInitiator::new(self.initiator_address.0, &self.rpc_provider);
		let recipient_bytes: [u8; 32] = recipient_address.0.try_into().unwrap();
		let call = contract.initiateBridgeTransfer(
			U256::from(amount.0),
			FixedBytes(recipient_bytes),
			FixedBytes(hash_lock.0),
			U256::from(time_lock.0),
		);
		let _ = send_transaction(
			call,
			&self.send_transaction_error_rules,
			self.num_tx_send_retries,
			self.gas_limit as u128,
		)
		.await;
		Ok(())
	}

	async fn complete_bridge_transfer(
		&mut self,
		bridge_transfer_id: BridgeTransferId<Self::Hash>,
		pre_image: HashLockPreImage,
	) -> BridgeContractInitiatorResult<()> {
		let pre_image: [u8; 32] = utils::vec_to_array(pre_image.0)?;
		let contract = AtomicBridgeInitiator::new(self.initiator_address.0, &self.rpc_provider);
		let call = contract
			.completeBridgeTransfer(FixedBytes(bridge_transfer_id.0), FixedBytes(pre_image));
		let _ = send_transaction(
			call,
			&self.send_transaction_error_rules,
			self.num_tx_send_retries,
			self.gas_limit as u128,
		)
		.await;
		Ok(())
	}

	async fn refund_bridge_transfer(
		&mut self,
		bridge_transfer_id: BridgeTransferId<Self::Hash>,
	) -> BridgeContractInitiatorResult<()> {
		let contract = AtomicBridgeInitiator::new(self.initiator_address.0, &self.rpc_provider);
		let call = contract.refundBridgeTransfer(FixedBytes(bridge_transfer_id.0));
		let _ = send_transaction(
			call,
			&self.send_transaction_error_rules,
			self.num_tx_send_retries,
			self.gas_limit as u128,
		)
		.await;
		Ok(())
	}

	async fn get_bridge_transfer_details(
		&mut self,
		bridge_transfer_id: BridgeTransferId<Self::Hash>,
	) -> BridgeContractInitiatorResult<Option<BridgeTransferDetails<Self::Address, Self::Hash>>> {
		let mapping_slot = U256::from(0); // the mapping is the zeroth slot in the contract
		let key = bridge_transfer_id.0;
		let storage_slot = self.calculate_storage_slot(key, mapping_slot);
		let storage: U256 = self
			.rpc_provider
			.get_storage_at(self.initiator_address.0, storage_slot)
			.await
			.map_err(|_| BridgeContractInitiatorError::GetMappingStorageError)?;
		let storage_bytes = storage.to_be_bytes::<32>();
		let mut storage_slice = &storage_bytes[..];
		let eth_details = EthBridgeTransferDetails::decode(&mut storage_slice)
			.map_err(|_| BridgeContractInitiatorError::DecodeStorageError)?;
		let details = BridgeTransferDetails {
			bridge_transfer_id,
			initiator_address: InitiatorAddress(eth_details.originator),
			recipient_address: RecipientAddress(eth_details.recipient.to_vec()),
			hash_lock: HashLock(eth_details.hash_lock),
			time_lock: TimeLock(eth_details.time_lock.wrapping_to::<u64>()),
			amount: Amount(eth_details.amount.wrapping_to::<u64>()),
		};

		Ok(Some(details))
	}
}

impl<P> EthClient<P> {
	fn calculate_storage_slot(&self, key: [u8; 32], mapping_slot: U256) -> U256 {
		#[derive(RlpEncodable)]
		struct SlotKey<'a> {
			key: &'a [u8; 32],
			mapping_slot: U256,
		}

		let slot_key = SlotKey { key: &key, mapping_slot };

		let mut buffer = Vec::new();
		slot_key.encode(&mut buffer);

		let hash = keccak(buffer);
		U256::from_be_slice(&hash.0)
	}
}