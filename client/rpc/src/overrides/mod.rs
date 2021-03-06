use ethereum::Block as EthereumBlock;
use ethereum_types::{H160, H256, U256};
use sp_runtime::traits::Block as BlockT;
use sp_api::{BlockId, ProvideRuntimeApi};
use sp_io::hashing::{twox_128, blake2_128};
use fp_rpc::TransactionStatus;
use std::{marker::PhantomData, sync::Arc};
use fp_rpc::EthereumRuntimeRPCApi;

mod schema_v1_override;

pub use fc_rpc_core::{EthApiServer, NetApiServer};
pub use schema_v1_override::SchemaV1Override;

/// Something that can fetch Ethereum-related data. This trait is quite similar to the runtime API,
/// and indeed oe implementation of it uses the runtime API.
/// Having this trait is useful because it allows optimized implementations that fetch data from a
/// State Backend with some assumptions about pallet-ethereum's storage schema. Using such an
/// optimized implementation avoids spawning a runtime and the overhead associated with it.
pub trait StorageOverride<Block: BlockT> {
	/// For a given account address, returns pallet_evm::AccountCodes.
	fn account_code_at(&self, block: &BlockId<Block>, address: H160) -> Option<Vec<u8>>;
	/// For a given account address and index, returns pallet_evm::AccountStorages.
	fn storage_at(&self, block: &BlockId<Block>, address: H160, index: U256) -> Option<H256>;
	/// Return the current block.
	fn current_block(&self, block: &BlockId<Block>) -> Option<EthereumBlock>;
	/// Return the current receipt.
	fn current_receipts(&self, block: &BlockId<Block>) -> Option<Vec<ethereum::Receipt>>;
	/// Return the current transaction status.
	fn current_transaction_statuses(&self, block: &BlockId<Block>) -> Option<Vec<TransactionStatus>>;
}

fn storage_prefix_build(module: &[u8], storage: &[u8]) -> Vec<u8> {
	[twox_128(module), twox_128(storage)].concat().to_vec()
}

fn blake2_128_extend(bytes: &[u8]) -> Vec<u8> {
	let mut ext: Vec<u8> = blake2_128(bytes).to_vec();
	ext.extend_from_slice(bytes);
	ext
}

/// A wrapper type for the Runtime API. This type implements `StorageOverride`, so it can be used
/// when calling the runtime API is desired but a `dyn StorageOverride` is required.
pub struct RuntimeApiStorageOverride<B: BlockT, C> {
	client: Arc<C>,
	_marker: PhantomData<B>,
}

impl<B, C> RuntimeApiStorageOverride<B, C> where
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B>,
	B: BlockT<Hash=H256> + Send + Sync + 'static,
	C: Send + Sync + 'static,
{
	pub fn new(client: Arc<C>) -> Self {
		Self { client, _marker: PhantomData }
	}
}

impl<Block, C> StorageOverride<Block> for RuntimeApiStorageOverride<Block, C>
where
	C: ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	Block: BlockT<Hash=H256> + Send + Sync + 'static,
	C: Send + Sync + 'static,
{
	/// For a given account address, returns pallet_evm::AccountCodes.
	fn account_code_at(&self, block: &BlockId<Block>, address: H160) -> Option<Vec<u8>> {
		self.client
			.runtime_api()
			.account_code_at(&block, address)
			.ok()
	}

	/// For a given account address and index, returns pallet_evm::AccountStorages.
	fn storage_at(&self, block: &BlockId<Block>, address: H160, index: U256) -> Option<H256> {
		self.client
			.runtime_api()
			.storage_at(&block, address, index)
			.ok()
	}

	/// Return the current block.
	fn current_block(&self, block: &BlockId<Block>) -> Option<EthereumBlock> {
		self.client
			.runtime_api()
			.current_block(&block)
			.ok()?
	}

	/// Return the current receipt.
	fn current_receipts(&self, block: &BlockId<Block>) -> Option<Vec<ethereum::Receipt>> {
		self.client.runtime_api().current_receipts(&block).ok()?
	}

	/// Return the current transaction status.
	fn current_transaction_statuses(&self, block: &BlockId<Block>) -> Option<Vec<TransactionStatus>> {
		self.client.runtime_api().current_transaction_statuses(&block).ok()?
	}
}
