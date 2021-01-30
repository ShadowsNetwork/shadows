#![cfg(test)]

use frame_support::{
	assert_noop, assert_ok,
	traits::{schedule::DispatchTime, OnFinalize, OnInitialize, OriginTrait},
};
use frame_system::RawOrigin;
use module_cdp_engine::LiquidationStrategy;
use module_support::{CDPTreasury, EXCHANGEManager, Price, Rate, Ratio, RiskManager};
use orml_authority::DelayedOrigin;
use orml_traits::{Change, MultiCurrency};
use shadows_runtime::{
	get_all_module_accounts, AccountId, AuthoritysOriginId, Balance, Balances, BlockNumber, Call, CreateClassDeposit,
	CurrencyId, DSWFModuleId, Event, GetNativeCurrencyId, NewAccountDeposit, Origin, OriginCaller, Perbill, Runtime,
	SevenDays, TokenSymbol, NFT,
};
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin},
	DispatchError, DispatchResult, FixedPointNumber,
};

const ORACLE1: [u8; 32] = [0u8; 32];
const ORACLE2: [u8; 32] = [1u8; 32];
const ORACLE3: [u8; 32] = [2u8; 32];

const ALICE: [u8; 32] = [4u8; 32];
const BOB: [u8; 32] = [5u8; 32];

pub type OracleModule = orml_oracle::Module<Runtime, orml_oracle::Instance1>;
pub type ExchangeModule = module_exchange::Module<Runtime>;
pub type CdpEngineModule = module_cdp_engine::Module<Runtime>;
pub type LendModule = module_lend::Module<Runtime>;
pub type CdpTreasuryModule = module_cdp_treasury::Module<Runtime>;
pub type SystemModule = frame_system::Module<Runtime>;
pub type EmergencyShutdownModule = module_emergency_shutdown::Module<Runtime>;
pub type AuctionManagerModule = module_auction_manager::Module<Runtime>;
pub type AuthorityModule = orml_authority::Module<Runtime>;
pub type Currencies = orml_currencies::Module<Runtime>;
pub type SchedulerModule = pallet_scheduler::Module<Runtime>;

fn run_to_block(n: u32) {
	while SystemModule::block_number() < n {
		SchedulerModule::on_finalize(SystemModule::block_number());
		SystemModule::set_block_number(SystemModule::block_number() + 1);
		SchedulerModule::on_initialize(SystemModule::block_number());
	}
}

fn last_event() -> Event {
	SystemModule::events().pop().expect("Event expected").event
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.endowed_accounts = endowed_accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let native_currency_id = GetNativeCurrencyId::get();
		let new_account_deposit = NewAccountDeposit::get();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.endowed_accounts
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == native_currency_id)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.chain(
					get_all_module_accounts()
						.iter()
						.map(|x| (x.clone(), new_account_deposit)),
				)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self
				.endowed_accounts
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != native_currency_id)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_membership::GenesisConfig::<Runtime, pallet_membership::Instance5> {
			members: vec![
				AccountId::from(ORACLE1),
				AccountId::from(ORACLE2),
				AccountId::from(ORACLE3),
			],
			phantom: Default::default(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

pub fn origin_of(account_id: AccountId) -> <Runtime as frame_system::Trait>::Origin {
	<Runtime as frame_system::Trait>::Origin::signed(account_id)
}

fn set_oracle_price(prices: Vec<(CurrencyId, Price)>) -> DispatchResult {
	OracleModule::on_finalize(0);
	assert_ok!(OracleModule::feed_values(
		origin_of(AccountId::from(ORACLE1)),
		prices.clone(),
	));
	assert_ok!(OracleModule::feed_values(
		origin_of(AccountId::from(ORACLE2)),
		prices.clone(),
	));
	assert_ok!(OracleModule::feed_values(origin_of(AccountId::from(ORACLE3)), prices,));
	Ok(())
}

fn amount(amount: u128) -> u128 {
	amount.saturating_mul(Price::accuracy())
}

#[test]
fn emergency_shutdown_and_cdp_treasury() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				2_000_000u128,
			),
			(
				AccountId::from(BOB),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				8_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				1_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::DOT),
				200_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::LDOT),
				40_000_000u128,
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				1_000_000
			));
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::DOT),
				200_000_000
			));
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::LDOT),
				40_000_000
			));
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				1_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::DOT)),
				200_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::LDOT)),
				40_000_000
			);

			assert_noop!(
				EmergencyShutdownModule::refund_collaterals(origin_of(AccountId::from(ALICE)), 1_000_000),
				module_emergency_shutdown::Error::<Runtime>::CanNotRefund,
			);
			assert_ok!(EmergencyShutdownModule::emergency_shutdown(
				<Runtime as frame_system::Trait>::Origin::root()
			));
			assert_ok!(EmergencyShutdownModule::open_collateral_refund(
				<Runtime as frame_system::Trait>::Origin::root()
			));
			assert_ok!(EmergencyShutdownModule::refund_collaterals(
				origin_of(AccountId::from(ALICE)),
				1_000_000
			));

			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				900_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::DOT)),
				180_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::LDOT)),
				36_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				1_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				100_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::DOT), &AccountId::from(ALICE)),
				20_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::LDOT), &AccountId::from(ALICE)),
				4_000_000
			);
		});
}

#[test]
fn liquidate_cdp() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(AccountId::from(ALICE), CurrencyId::Token(TokenSymbol::XBTC), amount(10)),
			(
				AccountId::from(BOB),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1_000_000),
			),
			(AccountId::from(BOB), CurrencyId::Token(TokenSymbol::XBTC), amount(101)),
		])
		.build()
		.execute_with(|| {
			SystemModule::set_block_number(1);
			assert_ok!(set_oracle_price(vec![(
				CurrencyId::Token(TokenSymbol::XBTC),
				Price::saturating_from_rational(10000, 1)
			)])); // 10000 usd

			assert_ok!(ExchangeModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(100),
				amount(1_000_000)
			));

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Trait>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(amount(1000000)),
			));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(10) as i128,
				amount(500_000) as i128
			));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1) as i128,
				amount(50_000) as i128
			));

			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(500_000)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(10)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).debit,
				amount(50_000)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).collateral,
				amount(1)
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), 0);
			assert_eq!(AuctionManagerModule::collateral_auctions(0), None);

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Trait>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
			));

			assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let liquidate_alice_xbtc_cdp_event =
				Event::module_cdp_engine(module_cdp_engine::RawEvent::LiquidateUnsafeCDP(
					CurrencyId::Token(TokenSymbol::XBTC),
					AccountId::from(ALICE),
					amount(10),
					amount(50_000),
					LiquidationStrategy::Auction,
				));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == liquidate_alice_xbtc_cdp_event));

			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				0
			);
			assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(50_000));

			assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let liquidate_bob_xbtc_cdp_event =
				Event::module_cdp_engine(module_cdp_engine::RawEvent::LiquidateUnsafeCDP(
					CurrencyId::Token(TokenSymbol::XBTC),
					AccountId::from(BOB),
					amount(1),
					amount(5_000),
					LiquidationStrategy::Exchange,
				));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == liquidate_bob_xbtc_cdp_event));

			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).debit,
				0
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).collateral,
				0
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(55_000));
			assert!(CdpTreasuryModule::surplus_pool() >= amount(5_000));
		});
}

#[test]
fn test_exchange_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				(1_000_000_000_000_000_000u128),
			),
		])
		.build()
		.execute_with(|| {
			SystemModule::set_block_number(1);

			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(0, 0)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				0
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(ALICE)
				),
				0
			);

			assert_noop!(
				ExchangeModule::add_liquidity(
					origin_of(AccountId::from(ALICE)),
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD),
					0,
					10000000
				),
				module_exchange::Error::<Runtime>::InvalidLiquidityIncrement,
			);

			assert_ok!(ExchangeModule::add_liquidity(
				origin_of(AccountId::from(ALICE)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				10000,
				10000000
			));

			let add_liquidity_event = Event::module_exchange(module_exchange::RawEvent::AddLiquidity(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				10000000,
				CurrencyId::Token(TokenSymbol::XBTC),
				10000,
				10000000,
			));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == add_liquidity_event));

			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10000, 10000000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10000000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(ALICE)
				),
				10000000
			);
			assert_ok!(ExchangeModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				1,
				1000
			));
			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10001, 10001000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10001000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(BOB)
				),
				1000
			);
			assert_noop!(
				ExchangeModule::add_liquidity(
					origin_of(AccountId::from(BOB)),
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD),
					1,
					999
				),
				module_exchange::Error::<Runtime>::InvalidLiquidityIncrement,
			);
			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10001, 10001000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10001000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(BOB)
				),
				1000
			);
			assert_ok!(ExchangeModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				2,
				1000
			));
			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10002, 10002000)
			);
			assert_ok!(ExchangeModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				1,
				1001
			));
			assert_eq!(
				ExchangeModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10003, 10003000)
			);

			assert_eq!(
				Currencies::total_issuance(CurrencyId::EXCHANGEShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10002998
			);
		});
}

#[test]
fn test_honzon_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1_000),
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(set_oracle_price(vec![(
				CurrencyId::Token(TokenSymbol::XBTC),
				Price::saturating_from_rational(1, 1)
			)]));

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Trait>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(amount(10000)),
			));
			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(100) as i128,
				amount(500) as i128
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				amount(50)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(500)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(100)
			);
			assert_eq!(
				CdpEngineModule::liquidate(
					<Runtime as frame_system::Trait>::Origin::none(),
					CurrencyId::Token(TokenSymbol::XBTC),
					AccountId::from(ALICE)
				)
				.is_ok(),
				false
			);
			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Trait>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
			));
			assert_ok!(CdpEngineModule::liquidate(
				<Runtime as frame_system::Trait>::Origin::none(),
				CurrencyId::Token(TokenSymbol::XBTC),
				AccountId::from(ALICE)
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				amount(50)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				0
			);
		});
}

#[test]
fn test_cdp_engine_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1000),
			),
		])
		.build()
		.execute_with(|| {
			SystemModule::set_block_number(1);
			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Trait>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(amount(10000)),
			));

			let new_collateral_params = CdpEngineModule::collateral_params(CurrencyId::Token(TokenSymbol::XBTC));

			assert_eq!(
				new_collateral_params.stability_fee,
				Some(Rate::saturating_from_rational(1, 100000))
			);
			assert_eq!(
				new_collateral_params.liquidation_ratio,
				Some(Ratio::saturating_from_rational(3, 2))
			);
			assert_eq!(
				new_collateral_params.liquidation_penalty,
				Some(Rate::saturating_from_rational(2, 10))
			);
			assert_eq!(
				new_collateral_params.required_collateral_ratio,
				Some(Ratio::saturating_from_rational(9, 5))
			);
			assert_eq!(new_collateral_params.maximum_total_debit_value, amount(10000));

			assert_eq!(
				CdpEngineModule::calculate_collateral_ratio(
					CurrencyId::Token(TokenSymbol::XBTC),
					100,
					50,
					Price::saturating_from_rational(1, 1)
				),
				Ratio::saturating_from_rational(100 * 10, 50)
			);

			assert_ok!(CdpEngineModule::check_debit_cap(
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(99999)
			));
			assert_eq!(
				CdpEngineModule::check_debit_cap(CurrencyId::Token(TokenSymbol::XBTC), amount(100001)).is_ok(),
				false
			);

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(100) as i128,
				0
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(100)
			);

			assert_noop!(
				CdpEngineModule::settle_cdp_has_debit(AccountId::from(ALICE), CurrencyId::Token(TokenSymbol::XBTC)),
				module_cdp_engine::Error::<Runtime>::NoDebitValue,
			);

			assert_ok!(set_oracle_price(vec![
				(
					CurrencyId::Token(TokenSymbol::AUSD),
					Price::saturating_from_rational(1, 1)
				),
				(
					CurrencyId::Token(TokenSymbol::XBTC),
					Price::saturating_from_rational(3, 1)
				)
			]));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				0,
				amount(100) as i128
			));
			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(100)
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), 0);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				0
			);
			assert_ok!(CdpEngineModule::settle_cdp_has_debit(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let settle_cdp_in_debit_event = Event::module_cdp_engine(module_cdp_engine::RawEvent::SettleCDPInDebit(
				CurrencyId::Token(TokenSymbol::XBTC),
				AccountId::from(ALICE),
			));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == settle_cdp_in_debit_event));

			assert_eq!(
				LendModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(10));
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				3333333333333333330
			);
		});
}

#[test]
fn test_authority_module() {
	const AUTHORITY_ORIGIN_ID: u8 = 30u8;

	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1000),
			),
			(
				DSWFModuleId::get().into_account(),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
		])
		.build()
		.execute_with(|| {
			let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::one()));
			let call = Call::Authority(orml_authority::Call::dispatch_as(
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone()),
			));

			// dispatch_as
			assert_ok!(AuthorityModule::dispatch_as(
				Origin::root(),
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone())
			));

			assert_noop!(
				AuthorityModule::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Root,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			assert_noop!(
				AuthorityModule::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::ShadowTreasury,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			// schedule_dispatch
			run_to_block(1);
			// DSWF transfer
			let transfer_call = Call::Currencies(orml_currencies::Call::transfer(
				AccountId::from(BOB).into(),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(500),
			));
			let dswf_call = Call::Authority(orml_authority::Call::dispatch_as(
				AuthoritysOriginId::DSWF,
				Box::new(transfer_call.clone()),
			));
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(2),
				0,
				true,
				Box::new(dswf_call.clone())
			));

			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(2),
				0,
				true,
				Box::new(call.clone())
			));

			let event = Event::orml_authority(orml_authority::RawEvent::Scheduled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				1,
			));
			assert_eq!(last_event(), event);

			run_to_block(2);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::Token(TokenSymbol::AUSD),
					&DSWFModuleId::get().into_account()
				),
				amount(500)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(BOB)),
				amount(500)
			);

			// delay < SevenDays
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(2, 1),
				Some([AUTHORITY_ORIGIN_ID, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
				Err(DispatchError::BadOrigin),
			));
			assert_eq!(last_event(), event);

			// delay = SevenDays
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 2),
				0,
				true,
				Box::new(call.clone())
			));

			run_to_block(SevenDays::get() + 2);
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(151202, 0),
				Some([AUTHORITY_ORIGIN_ID, 160, 78, 2, 0, 0, 0, 2, 0, 0, 0].to_vec()),
				Ok(()),
			));
			assert_eq!(last_event(), event);

			// with_delayed_origin = false
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 3),
				0,
				false,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::RawEvent::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				3,
			));
			assert_eq!(last_event(), event);

			run_to_block(SevenDays::get() + 3);
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(151203, 0),
				Some([0, 0, 3, 0, 0, 0].to_vec()),
				Ok(()),
			));
			assert_eq!(last_event(), event);

			// fast_track_scheduled_dispatch
			assert_ok!(AuthorityModule::fast_track_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				0,
				DispatchTime::At(SevenDays::get() + 4),
			));

			// delay_scheduled_dispatch
			assert_ok!(AuthorityModule::delay_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				0,
				5,
			));

			// cancel_scheduled_dispatch
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 4),
				0,
				true,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::RawEvent::Scheduled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				4,
			));
			assert_eq!(last_event(), event);

			let schedule_origin = {
				let origin: <Runtime as orml_authority::Trait>::Origin = From::from(Origin::root());
				let origin: <Runtime as orml_authority::Trait>::Origin = From::from(DelayedOrigin::<
					BlockNumber,
					<Runtime as orml_authority::Trait>::PalletsOrigin,
				> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
				origin
			};

			let pallets_origin = schedule_origin.caller().clone();
			assert_ok!(AuthorityModule::cancel_scheduled_dispatch(
				Origin::root(),
				pallets_origin,
				4
			));
			let event = Event::orml_authority(orml_authority::RawEvent::Cancelled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				4,
			));
			assert_eq!(last_event(), event);

			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 5),
				0,
				false,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::RawEvent::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				5,
			));
			assert_eq!(last_event(), event);

			assert_ok!(AuthorityModule::cancel_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				5
			));
			let event = Event::orml_authority(orml_authority::RawEvent::Cancelled(
				OriginCaller::system(RawOrigin::Root),
				5,
			));
			assert_eq!(last_event(), event);
		});
}

#[test]
fn test_nft_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				GetNativeCurrencyId::get(),
				NewAccountDeposit::get(),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::DOS),
				amount(1000),
			),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(AccountId::from(ALICE)), amount(1000));
			assert_ok!(NFT::create_class(
				origin_of(AccountId::from(ALICE)),
				vec![1],
				module_nft::Properties(module_nft::ClassProperty::Transferable | module_nft::ClassProperty::Burnable)
			));
			assert_ok!(NFT::mint(
				origin_of(AccountId::from(ALICE)),
				AccountId::from(BOB),
				0,
				vec![1],
				1
			));
			assert_ok!(NFT::burn(origin_of(AccountId::from(BOB)), (0, 0)));
			assert_eq!(Balances::free_balance(AccountId::from(BOB)), 0);
			assert_ok!(NFT::destroy_class(
				origin_of(AccountId::from(ALICE)),
				0,
				AccountId::from(BOB)
			));
			assert_eq!(Balances::free_balance(AccountId::from(BOB)), CreateClassDeposit::get());
			assert_eq!(
				Balances::reserved_balance(AccountId::from(BOB)),
				NewAccountDeposit::get()
			);
			// CreateClassDeposit::get() + NewAccountDeposit::get() = 6000000000000000
			assert_eq!(Balances::free_balance(AccountId::from(ALICE)), 999994000000000000000);
		});
}
