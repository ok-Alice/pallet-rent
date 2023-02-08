#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	use frame_support::{
		ensure,
		traits::{Currency, Get, Randomness},
	};

	use scale_info::prelude::vec;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Currency: Currency<Self::AccountId>;
		type CollectionRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type MaximumOwned: Get<u32>;

		#[pallet::constant]
		type MaximumRentablesPerBlock: Get<u32>;

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
		pub lessor: T::AccountId,
		pub current_lessee: Option<T::AccountId>,
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

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct RentalPeriodConfig<T: Config> {
		collectible: [u8; 16],
		period: T::BlockNumber,
		recurring: bool,
	}

	/// Track rental periods.
	#[pallet::storage]
	pub(super) type RentalPeriods<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::BlockNumber,
		BoundedVec<RentalPeriodConfig<T>, T::MaximumRentablesPerBlock>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new collectible was successfully created.
		CollectibleCreated {
			collectible: [u8; 16],
			lessor: T::AccountId,
			current_lessee: Option<T::AccountId>,
		},
		/// A collectible was successfully transferred.
		TransferSucceeded {
			from: T::AccountId,
			to: T::AccountId,
			collectible: [u8; 16],
		},
		/// The price of a collectible was successfully set.
		RentSet {
			collectible: [u8; 16],
			price_per_block: BalanceOf<T>,
		},
		/// A collectible was successfully rented.
		Rented {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
			total_rent_price: BalanceOf<T>,
		},
		RentalPeriodEnded {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
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
	impl<T: Config> Pallet<T> {
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
			ensure!(collectible.lessor == sender, Error::<T>::NotLessor);

			collectible.price_per_block = Some(price_per_block);

			CollectibleMap::<T>::insert(&unique_id, collectible);
			RentableCollectibles::<T>::put(unique_id);

			Self::deposit_event(Event::RentSet { collectible: unique_id, price_per_block });
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn rent(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			blocks: u32,
			recurring: bool,
		) -> DispatchResult {
			let lessee = ensure_signed(origin)?;

			ensure!(blocks >= 30, Error::<T>::RentalPeriodTooShort);
			ensure!(blocks <= 90, Error::<T>::RentalPeriodTooLong);

			let collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessor != lessee, Error::<T>::CannotRentOwnCollectible);
			ensure!(collectible.current_lessee.is_none(), Error::<T>::RentNotAvailable);

			Self::do_rent_collectible(unique_id, lessee, blocks, recurring)?;
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
				lessor: lessor.clone(),
				current_lessee: None,
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
				lessor: lessor.clone(),
				current_lessee: None,
			});

			Ok(unique_id)
		}

		fn do_rent_collectible(
			unique_id: [u8; 16],
			lessee: T::AccountId,
			blocks: u32,
			recurring: bool,
		) -> DispatchResult {
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let lessor = &collectible.lessor;
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

			collectible.current_lessee = Some(lessee.clone());

			let block_number: T::BlockNumber = blocks.into();

			let next_rental_period: T::BlockNumber =
				block_number + frame_system::Pallet::<T>::block_number();

			let rental_config = RentalPeriodConfig {
				collectible: unique_id,
				period: next_rental_period,
				recurring,
			};

			Self::insert_rental_period(rental_config).unwrap();

			CollectibleMap::<T>::insert(&unique_id, collectible);
			Ok(())
		}

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

		fn insert_rental_period(rental_config: RentalPeriodConfig<T>) -> DispatchResult {
			let mut block_number: T::BlockNumber = rental_config.period;

			let mut rental_periods = RentalPeriods::<T>::get(block_number);

			// try to append the collectible to the rental period
			// if it fails (because the rental period is already full), increment the rental period
			// and try again
			while let Err(_) = rental_periods.try_append(&mut vec![rental_config.clone()]) {
				let next_block_number: T::BlockNumber = (1 as u32).into();
				block_number = frame_system::Pallet::<T>::block_number() + next_block_number;
				rental_periods = RentalPeriods::<T>::get(block_number);
			}

			RentalPeriods::<T>::insert(block_number, rental_periods);
			Ok(())
		}

		fn process_rental_periods(n: T::BlockNumber) {
			let rental_periods = RentalPeriods::<T>::get(n);

			for rental_config in rental_periods {
				let collectible_id = rental_config.collectible;

				let collectible = CollectibleMap::<T>::get(&collectible_id)
					.ok_or(Error::<T>::NoCollectible)
					.unwrap();

				let lessor = collectible.lessor;
				let lessee = collectible.current_lessee.unwrap();

				if n != rental_config.period || !rental_config.recurring {
					Self::deposit_event(Event::RentalPeriodEnded {
						lessee: lessee.clone(),
						lessor: lessor.clone(),
						collectible: collectible_id,
					});

					continue
				}

				let total_rent_price = collectible.price_per_block.unwrap();

				// Mutating state with a balance transfer, so nothing is allowed to fail after
				// this.
				T::Currency::transfer(
					&lessee,
					&lessor,
					total_rent_price,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				)
				.unwrap();

				if rental_config.recurring {
					Self::insert_rental_period(rental_config).unwrap();
				}
			}

			RentalPeriods::<T>::remove(n);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::process_rental_periods(n);

			// TODO: Calculate weight
			Weight::from_parts(0, 0)
		}
	}
}
