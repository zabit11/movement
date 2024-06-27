use futures::Stream;

use crate::types::{BridgeTransferDetails, BridgeTransferId, CompletedDetails, LockDetails};

#[derive(Debug, PartialEq, Eq)]
pub enum BridgeContractInitiatorEvent<A, H> {
	Initiated(BridgeTransferDetails<A, H>),
	Completed(BridgeTransferId<H>),
	Refunded(BridgeTransferId<H>),
}

impl<A, H> BridgeContractInitiatorEvent<A, H> {
	pub fn bridge_transfer_id(&self) -> &BridgeTransferId<H> {
		match self {
			Self::Initiated(details) => &details.bridge_transfer_id,
			Self::Completed(id) | Self::Refunded(id) => id,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
pub enum BridgeContractCounterpartyEvent<A, H> {
	Locked(LockDetails<A, H>),
	Completed(CompletedDetails<A, H>),
}

pub trait BridgeContractInitiatorMonitoring:
	Stream<Item = BridgeContractInitiatorEvent<Self::Address, Self::Hash>>
{
	type Address;
	type Hash;
}

pub trait BridgeContractCounterpartyMonitoring:
	Stream<Item = BridgeContractCounterpartyEvent<Self::Address, Self::Hash>>
{
	type Address;
	type Hash;
}