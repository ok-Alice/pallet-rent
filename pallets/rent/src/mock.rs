use crate::{self as pallet_rent, Collectibles, LessorCollectibles};
use frame_support::{
	construct_runtime, parameter_types, sp_io,
	sp_runtime::{
		self,
		app_crypto::sp_core,
		generic,
		traits::{BlakeTwo256, IdentifyAccount, One, Verify},
		MultiSignature,
	},
	sp_tracing,
	traits::{ConstU32, ConstU64, ConstU8, Currency, Hooks},
	weights::IdentityFee,
};
use pallet_balances::AccountData;
use pallet_transaction_payment::{ConstFeeMultiplier, CurrencyAdapter, Multiplier};

use crate::Config;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Test>,
	frame_system::CheckSpecVersion<Test>,
	frame_system::CheckTxVersion<Test>,
	frame_system::CheckGenesis<Test>,
	frame_system::CheckEra<Test>,
	frame_system::CheckNonce<Test>,
	frame_system::CheckWeight<Test>,
	pallet_transaction_payment::ChargeTransactionPayment<Test>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// Balance of an account.
pub type Balance = u64;

parameter_types! {
	pub FeeMultiplier: Multiplier = Multiplier::one();
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Rent: pallet_rent::{Pallet, Call, Storage, Event<T>},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage},
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = sp_core::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = u64;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Header = sp_runtime::testing::Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CollectionRandomness = RandomnessCollectiveFlip;
	type MaximumOwned = frame_support::pallet_prelude::ConstU32<100>;
	type MaximumRentablesPerBlock = frame_support::pallet_prelude::ConstU32<100>;
}

pub const EXISTENTIAL_DEPOSIT: u64 = 500;

impl pallet_balances::Config for Test {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Test>;
}

impl pallet_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
}

impl pallet_randomness_collective_flip::Config for Test {}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<5>;
	type WeightInfo = ();
}

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		let _ = pallet_balances::GenesisConfig::<Test> {
			balances: vec![(1, 1000000000), (2, 1000000000), (3, 1000000000)],
		}
		.assimilate_storage(&mut storage);

		let mut ext = sp_io::TestExternalities::new(storage);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
		sp_tracing::try_init_simple();
		let mut ext = self.build();
		ext.execute_with(test);
	}
}

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

/// Progress to the given block, triggering session and era changes as we progress.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
	<Rent as Hooks<u64>>::on_finalize(System::block_number());
	for b in (System::block_number() + 1)..=n.into() {
		System::set_block_number(b);
		Rent::on_initialize(b);
		Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
		if b != n as u64 {
			<Rent as Hooks<u64>>::on_finalize(System::block_number());
		}
	}
}

pub(crate) fn add_collectible(
	collectible_id: [u8; 16],
	lessor: u64,
	lessee: Option<u64>,
	rentable: bool,
	price_per_block: Option<BalanceOf<Test>>,
	minimum_rental_period: Option<BlockNumber>,
	maximum_rental_period: Option<BlockNumber>,
) {
	Collectibles::<Test>::insert(
		collectible_id,
		crate::Collectible {
			collectible_id,
			lessor,
			lessee,
			rentable,
			price_per_block,
			minimum_rental_period,
			maximum_rental_period,
		},
	);

	let mut lessor_collectibles = LessorCollectibles::<Test>::get(&lessor).unwrap_or_default();
	lessor_collectibles.try_push(collectible_id).unwrap();
	LessorCollectibles::<Test>::insert(&lessor, lessor_collectibles);
}
