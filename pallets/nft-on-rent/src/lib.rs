#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use frame_support::{
		inherent::Vec,
		traits::{Currency, Randomness},
	};

	use pallet_timestamp::{self as timestamp};

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + timestamp::Config + pallet_scheduler::Config {
		type Currency: Currency<Self::AccountId>;
		type CollectionRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type MaximumOwned: Get<u32>;

		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Collectible<T: Config> {
		// Unsigned integers of 16 bytes to represent a unique identifier
		pub unique_id: [u8; 16],
		// `None` assumes not for sale
		pub price_per_block: Option<BalanceOf<T>>,
		pub original_lessor: T::AccountId,
		pub current_lessor: Option<T::AccountId>,
	}

	/// Maps the Collectible struct to the unique_id.
	#[pallet::storage]
	pub(super) type CollectibleMap<T: Config> =
		StorageMap<_, Twox64Concat, [u8; 16], Collectible<T>>;

	/// Track the collectibles owned by each account.
	#[pallet::storage]
	pub(super) type LessorOfCollectibles<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaximumOwned>,
		ValueQuery,
	>;

	/// Track rentable collectibles
	#[pallet::storage]
	pub type RentableCollectibles<T> = StorageValue<_, [u8; 16], ValueQuery>;

	/// Track the collectibles rented by each account.
	#[pallet::storage]
	pub(super) type LesseeOfCollectibles<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaximumOwned>,
		ValueQuery,
	>;

	/// Track rental periods.
	#[pallet::storage]
	pub(super) type RentalPeriods<T: Config> =
		StorageMap<_, Twox64Concat, T::Moment, [u8; 16], ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new collectible was successfully created.
		CollectibleCreated { collectible: [u8; 16], current_lessor: T::AccountId },
		/// A collectible was successfully transferred.
		TransferSucceeded { from: T::AccountId, to: T::AccountId, collectible: [u8; 16] },
		/// The price of a collectible was successfully set.
		PriceSet { collectible: [u8; 16], price_per_block: BalanceOf<T> },
		/// A collectible was successfully rented.
		Rented {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
			total_rent_price: BalanceOf<T>,
		},
		RentalPeriodProcessed {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
			price: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Each collectible must have a unique identifier
		DuplicateCollectible,
		/// The collectible doesn't exist
		NoCollectible,
		/// You are not the lessor
		NotLessor,
		/// The collectible is already rented.
		AlreadyRented,
		/// The accounds can't exceed the maximum number of collectibles.
		TooManyCollectibles,
		/// The collectible is not available for rent.
		RentNotAvailable,
		/// The lessor of the collectible cannot rent it.
		CannotRentOwnCollectible,
		RentalPeriodTooShort,
		RentalPeriodTooLong,
	}

	// Pallet callable functions
	#[pallet::call]
	impl<T: Config> Pallet<T>
	// where
	// 	<T as pallet_scheduler::Config>::RuntimeCall: From<Call<T>>,
	{
		/// Create a new unique collectible.
		///
		/// The actual collectible creation is done in the `mint()` function.
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn mint(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let collectible_gen_unique_id = Self::_gen_unique_id();

			Self::do_mint(&sender, collectible_gen_unique_id)?;

			Ok(())
		}

		/// Update the collectible price and write to storage.
		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn set_rentable(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			price_per_block: BalanceOf<T>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.original_lessor == sender, Error::<T>::NotLessor);

			collectible.price_per_block = Some(price_per_block);

			CollectibleMap::<T>::insert(&unique_id, collectible);
			RentableCollectibles::<T>::put(unique_id);

			Self::deposit_event(Event::PriceSet { collectible: unique_id, price_per_block });
			Ok(())
		}

		/// Attempt to implement the pallet_scheduler
		// #[pallet::weight(0)]
		// #[pallet::call_index(2)]
		// pub fn rent_collectible(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
		// 	let mut name: [u8; 32] = [0; 32];
		// 	name[..16].copy_from_slice(&unique_id);
		// 	name[16..].copy_from_slice(b"rental_period");

		// 	let task_name = frame_support::traits::schedule::v3::TaskName::from(name);

		// 	<pallet_scheduler::Pallet<T>>::schedule_named(
		// 		origin,
		// 		task_name,
		// 		<frame_system::Pallet<T>>::block_number(),
		// 		None,
		// 		0,
		// 		Box::new(Call::<T>::something { unique_id }.into()),
		// 	)?;

		// 	Ok(())
		// }

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn rent(origin: OriginFor<T>, unique_id: [u8; 16], blocks: u32) -> DispatchResult {
			let lessee = ensure_signed(origin)?;

			ensure!(blocks >= 30, Error::<T>::RentalPeriodTooShort);
			ensure!(blocks <= 90, Error::<T>::RentalPeriodTooLong);

			let collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.original_lessor != lessee, Error::<T>::CannotRentOwnCollectible);
			ensure!(collectible.current_lessor.is_some(), Error::<T>::RentNotAvailable);

			Self::do_rent_collectible(unique_id, lessee, blocks)?;
			Ok(())
		}
	}

	// Pallet internal functions
	impl<T: Config> Pallet<T> {
		// Function to mint a collectible
		pub fn do_mint(
			lessor: &T::AccountId,
			unique_id: [u8; 16],
		) -> Result<[u8; 16], DispatchError> {
			let collectible = Collectible::<T> {
				unique_id,
				price_per_block: None,
				original_lessor: lessor.clone(),
				current_lessor: Some(lessor.clone()),
			};

			ensure!(
				!CollectibleMap::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateCollectible
			);

			LessorOfCollectibles::<T>::try_append(lessor, collectible.unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;

			CollectibleMap::<T>::insert(collectible.unique_id, collectible);

			Self::deposit_event(Event::CollectibleCreated {
				collectible: unique_id,
				current_lessor: lessor.clone(),
			});

			Ok(unique_id)
		}

		fn do_rent_collectible(
			unique_id: [u8; 16],
			lessee: T::AccountId,
			blocks: u32,
		) -> DispatchResult {
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let lessor = &collectible.original_lessor;
			let lessee = lessee.clone();

			let total_rent_price = collectible.price_per_block.unwrap() * blocks.into();

			// Mutating state with a balance transfer, so nothing is allowed to fail after this.
			T::Currency::transfer(
				&lessee,
				&lessor,
				total_rent_price,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			Self::deposit_event(Event::Rented {
				lessee: lessee.clone(),
				lessor: lessor.clone(),
				collectible: unique_id,
				total_rent_price,
			});

			collectible.current_lessor = Some(lessee.clone());

			let now = <timestamp::Pallet<T>>::get();
			let next_rental_period: T::Moment = now + T::Moment::from(100u32);

			CollectibleMap::<T>::insert(&unique_id, collectible);
			LesseeOfCollectibles::<T>::try_append(&lessee, unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;
			RentalPeriods::<T>::insert(next_rental_period, &unique_id);

			Ok(())
		}

		/// This function is for processing periodic rent payments executed from the `on_finalize`
		/// triggered hook
		// fn process_rental_periods() -> DispatchResult {
		// 	let now = <timestamp::Pallet<T>>::get();
		// 	let rental_periods =
		// 		RentalPeriods::<T>::iter().filter(|(k, _)| *k <= now).collect::<Vec<_>>();

		// 	rental_periods.iter().for_each(|(k, v)| {
		// 		let collectible = CollectibleMap::<T>::get(v).unwrap();
		// 		let lessee = collectible.current_lessor.unwrap();
		// 		let lessor = collectible.original_lessor;
		// 		let price = collectible.price_per_block.unwrap();

		// 		T::Currency::transfer(
		// 			&lessee,
		// 			&lessor,
		// 			price,
		// 			frame_support::traits::ExistenceRequirement::KeepAlive,
		// 		)
		// 		.unwrap();

		// 		RentalPeriods::<T>::remove(k);

		// 		let next_rental_period: T::Moment = now + T::Moment::from(100u32);
		// 		RentalPeriods::<T>::insert(next_rental_period, v);

		// 		Self::deposit_event(Event::RentalPeriodProcessed {
		// 			lessee,
		// 			lessor,
		// 			collectible: *v,
		// 			price,
		// 		});
		// 	});

		// 	Ok(())
		// }

		// Generates and returns the unique_id
		fn _gen_unique_id() -> [u8; 16] {
			let random = T::CollectionRandomness::random(&b"unique_id"[..]).0;

			// Create randomness payload. Multiple collectibles can be generated in the same block,
			// retaining uniqueness.
			let unique_payload = (
				random,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			let encoded_payload = unique_payload.encode();
			frame_support::Hashable::blake2_128(&encoded_payload)
		}
	}

	/// This hook is called after a block is finalized and will execute the periodic rent payments method
	// #[pallet::hooks]
	// impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
	// 	fn on_finalize(_n: T::BlockNumber) {
	// 		Self::process_rental_periods().unwrap();
	// 	}
	// }
}
