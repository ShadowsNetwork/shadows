//! Mocks for the debt treasury module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use primitives::{TokenSymbol, TradingPair};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use sp_std::cell::RefCell;

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i64;
pub type AuctionId = u32;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const DOS: CurrencyId = CurrencyId::Token(TokenSymbol::DOS);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod debt_treasury {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		debt_treasury,
		orml_tokens<T>,
		pallet_balances<T>,
		orml_currencies<T>,
		exchange<T>,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}
pub type PalletBalances = pallet_balances::Module<Runtime>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = DOS;
}

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Runtime>;

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub const TradingPathLimit: usize = 3;
	pub EnabledTradingPairs : Vec<TradingPair> = vec![TradingPair::new(AUSD, BTC)];
	pub const EXCHANGEModuleId: ModuleId = ModuleId(*b"dos/dexm");
}

impl exchange::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type EnabledTradingPairs = EnabledTradingPairs;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = EXCHANGEModuleId;
	type WeightInfo = ();
}
pub type EXCHANGEModule = exchange::Module<Runtime>;

thread_local! {
	pub static TOTAL_COLLATERAL_AUCTION: RefCell<u32> = RefCell::new(0);
	pub static TOTAL_COLLATERAL_IN_AUCTION: RefCell<Balance> = RefCell::new(0);
	pub static TOTAL_DEBIT_AUCTION: RefCell<u32> = RefCell::new(0);
	pub static TOTAL_SURPLUS_AUCTION: RefCell<u32> = RefCell::new(0);
}

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		_refund_recipient: &AccountId,
		_currency_id: Self::CurrencyId,
		amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut() += 1);
		TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut() += amount);
		Ok(())
	}

	fn new_debit_auction(_amount: Self::Balance, _fix: Self::Balance) -> DispatchResult {
		TOTAL_DEBIT_AUCTION.with(|v| *v.borrow_mut() += 1);
		Ok(())
	}

	fn new_surplus_auction(_amount: Self::Balance) -> DispatchResult {
		TOTAL_SURPLUS_AUCTION.with(|v| *v.borrow_mut() += 1);
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		Ok(())
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut())
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_debit_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Default::default()
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
	pub const MaxAuctionsCount: u32 = 5;
}

parameter_types! {
	pub const DEBTTreasuryModuleId: ModuleId = ModuleId(*b"dos/cdpt");
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type EXCHANGE = EXCHANGEModule;
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = DEBTTreasuryModuleId;
	type WeightInfo = ();
}
pub type DEBTTreasuryModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, AUSD, 1000),
				(ALICE, BTC, 1000),
				(BOB, AUSD, 1000),
				(BOB, BTC, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}