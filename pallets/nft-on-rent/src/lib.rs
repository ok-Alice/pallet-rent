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
		pub rentable: bool,
	}

	/// Maps the Collectible struct to the unique_id.
	#[pallet::storage]
	pub(super) type CollectibleMap<T: Config> =
		StorageMap<_, Twox64Concat, [u8; 16], Collectible<T>>;

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct RentalPeriodConfig<T: Config> {
		collectible: [u8; 16],
		rental_periodic_interval: T::BlockNumber,
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
		TransferSucceeded { from: T::AccountId, to: T::AccountId, collectible: [u8; 16] },
		/// The price of a collectible was successfully set.
		RentMadeAvailable { collectible: [u8; 16], price_per_block: BalanceOf<T> },
		/// A collectible was successfully rented.
		RentPayed {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
			total_rent_price: BalanceOf<T>,
			rental_periodic_interval: T::BlockNumber,
		},
		/// A rental period was successfully added.
		RentalPeriodAdded {
			collectible: [u8; 16],
			next_rent_block: T::BlockNumber,
			recurring: bool,
		},
		/// A rental period was successfully ended.
		RentalPeriodEnded { lessor: T::AccountId, lessee: T::AccountId, collectible: [u8; 16] },
		/// Collectible rent made unavailable.
		RentMadeUnavailable { collectible: [u8; 16] },
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
		/// The rentale period is too short.
		RentalPeriodTooShort,
		/// The rentale period is too long.
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
			collectible.rentable = true;

			CollectibleMap::<T>::insert(&unique_id, collectible);

			Self::deposit_event(Event::RentMadeAvailable {
				collectible: unique_id,
				price_per_block,
			});
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

			ensure!(blocks >= 3, Error::<T>::RentalPeriodTooShort);
			ensure!(blocks <= 90, Error::<T>::RentalPeriodTooLong);

			let collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessor != lessee, Error::<T>::CannotRentOwnCollectible);
			ensure!(collectible.current_lessee.is_none(), Error::<T>::RentNotAvailable);
			ensure!(collectible.rentable, Error::<T>::RentNotAvailable);

			Self::do_rent_collectible(unique_id, lessee, blocks, recurring)?;
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(4)]
		pub fn set_unrentable(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessor == sender, Error::<T>::NotLessor);

			collectible.rentable = false;

			CollectibleMap::<T>::insert(&unique_id, collectible);

			Self::deposit_event(Event::RentMadeUnavailable { collectible: unique_id });
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
				rentable: false,
			};

			ensure!(
				!CollectibleMap::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateCollectible
			);

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
			rent_periodic_interval: u32,
			recurring: bool,
		) -> DispatchResult {
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let lessor = &collectible.lessor;
			let lessee = lessee.clone();

			let total_rent_price =
				collectible.price_per_block.unwrap() * rent_periodic_interval.into();

			// Mutating state with a balance transfer, so nothing is allowed to fail after this.
			T::Currency::transfer(
				&lessee,
				&lessor,
				total_rent_price,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			Self::deposit_event(Event::RentPayed {
				lessee: lessee.clone(),
				lessor: lessor.clone(),
				collectible: unique_id,
				total_rent_price,
				rental_periodic_interval: rent_periodic_interval.into(),
			});

			collectible.current_lessee = Some(lessee.clone());

			let rental_config = RentalPeriodConfig {
				collectible: unique_id,
				rental_periodic_interval: rent_periodic_interval.into(),
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
			let mut block_number =
				rental_config.rental_periodic_interval + frame_system::Pallet::<T>::block_number();

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

			Self::deposit_event(Event::RentalPeriodAdded {
				collectible: rental_config.collectible,
				next_rent_block: block_number,
				recurring: rental_config.recurring,
			});

			Ok(())
		}

		fn process_rental_periods(n: T::BlockNumber) {
			let rental_periods = RentalPeriods::<T>::get(n);

			for rental_config in rental_periods {
				let collectible_id = rental_config.collectible;

				let mut collectible = CollectibleMap::<T>::get(&collectible_id)
					.ok_or(Error::<T>::NoCollectible)
					.unwrap();

				let lessor = collectible.lessor.clone();
				let lessee = collectible.current_lessee.unwrap();

				if !collectible.rentable || !rental_config.recurring {
					collectible.current_lessee = None;

					CollectibleMap::<T>::insert(&collectible_id, collectible);

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

				Self::deposit_event(Event::RentPayed {
					lessee: lessee.clone(),
					lessor: lessor.clone(),
					collectible: collectible_id,
					total_rent_price,
					rental_periodic_interval: rental_config.rental_periodic_interval.into(),
				});

				// Add the rental period again if recurring
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
