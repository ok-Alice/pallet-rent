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
	pub trait Config: frame_system::Config + timestamp::Config {
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
		pub price: Option<BalanceOf<T>>,
		pub owner: T::AccountId,
		pub renter: Option<T::AccountId>,
	}

	/// Maps the Collectible struct to the unique_id.
	#[pallet::storage]
	pub(super) type CollectibleMap<T: Config> =
		StorageMap<_, Twox64Concat, [u8; 16], Collectible<T>>;

	/// Track the collectibles owned by each account.
	#[pallet::storage]
	pub(super) type OwnerOfCollectibles<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaximumOwned>,
		ValueQuery,
	>;

	/// Track the collectibles rented by each account.
	#[pallet::storage]
	pub(super) type RenterOfCollectibles<T: Config> = StorageMap<
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
		CollectibleCreated { collectible: [u8; 16], owner: T::AccountId },
		/// A collectible was successfully transferred.
		TransferSucceeded { from: T::AccountId, to: T::AccountId, collectible: [u8; 16] },
		/// The price of a collectible was successfully set.
		PriceSet { collectible: [u8; 16], price: Option<BalanceOf<T>> },
		/// A collectible was successfully rented.
		Rented {
			owner: T::AccountId,
			renter: T::AccountId,
			collectible: [u8; 16],
			price: BalanceOf<T>,
		},
		RentalPeriodProcessed {
			owner: T::AccountId,
			renter: T::AccountId,
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
		/// You are not the owner
		NotOwner,
		/// The collectible is not for sale.
		NotForRent,
		/// The collectible is already rented.
		AlreadyRented,
		/// The accounds can't exceed the maximum number of collectibles.
		TooManyCollectibles,
	}

	// Pallet callable functions
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new unique collectible.
		///
		/// The actual collectible creation is done in the `mint()` function.
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn create_collectible(
			origin: OriginFor<T>,
			price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let collectible_gen_unique_id = Self::gen_unique_id();

			Self::mint(&sender, collectible_gen_unique_id, price)?;

			Ok(())
		}

		/// Update the collectible price and write to storage.
		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn set_price(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.owner == sender, Error::<T>::NotOwner);

			collectible.price = new_price;
			CollectibleMap::<T>::insert(&unique_id, collectible);

			Self::deposit_event(Event::PriceSet { collectible: unique_id, price: new_price });
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(2)]
		pub fn rent_collectible(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let renter = ensure_signed(origin)?;

			let collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.owner != renter, Error::<T>::NotOwner);
			ensure!(collectible.price.is_some(), Error::<T>::NotForRent);
			ensure!(collectible.renter.is_none(), Error::<T>::AlreadyRented);

			Self::do_rent_collectible(unique_id, renter)?;
			Ok(())
		}
	}

	// Pallet internal functions
	impl<T: Config> Pallet<T> {
		// Function to mint a collectible
		pub fn mint(
			owner: &T::AccountId,
			unique_id: [u8; 16],
			price: Option<BalanceOf<T>>,
		) -> Result<[u8; 16], DispatchError> {
			let collectible =
				Collectible::<T> { unique_id, price, owner: owner.clone(), renter: None };

			ensure!(
				!CollectibleMap::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateCollectible
			);

			OwnerOfCollectibles::<T>::try_append(owner, collectible.unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;

			CollectibleMap::<T>::insert(collectible.unique_id, collectible);

			Self::deposit_event(Event::CollectibleCreated {
				collectible: unique_id,
				owner: owner.clone(),
			});

			Ok(unique_id)
		}

		// Generates and returns the unique_id
		fn gen_unique_id() -> [u8; 16] {
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

		fn do_rent_collectible(unique_id: [u8; 16], renter: T::AccountId) -> DispatchResult {
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let owner = &collectible.owner;
			let renter = renter.clone();

			// Mutating state with a balance transfer, so nothing is allowed to fail after this.
			if let Some(price) = collectible.price {
				T::Currency::transfer(
					&renter,
					&owner,
					price,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				)?;
				Self::deposit_event(Event::Rented {
					renter: renter.clone(),
					owner: owner.clone(),
					collectible: unique_id,
					price,
				});
			} else {
				return Err(Error::<T>::NotForRent.into())
			}

			collectible.renter = Some(renter.clone());

			let now = <timestamp::Pallet<T>>::get();
			let next_rental_period: T::Moment = now + T::Moment::from(100u32);

			CollectibleMap::<T>::insert(&unique_id, collectible);
			RenterOfCollectibles::<T>::try_append(&renter, unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;
			RentalPeriods::<T>::insert(next_rental_period, &unique_id);

			Ok(())
		}

		fn process_rental_periods() -> DispatchResult {
			let now = <timestamp::Pallet<T>>::get();
			let rental_periods =
				RentalPeriods::<T>::iter().filter(|(k, _)| *k <= now).collect::<Vec<_>>();

			rental_periods.iter().for_each(|(k, v)| {
				let collectible = CollectibleMap::<T>::get(v).unwrap();
				let renter = collectible.renter.unwrap();
				let owner = collectible.owner;
				let price = collectible.price.unwrap();

				T::Currency::transfer(
					&renter,
					&owner,
					price,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				)
				.unwrap();

				RentalPeriods::<T>::remove(k);

				let next_rental_period: T::Moment = now + T::Moment::from(100u32);
				RentalPeriods::<T>::insert(next_rental_period, v);

				Self::deposit_event(Event::RentalPeriodProcessed {
					renter,
					owner,
					collectible: *v,
					price,
				});
			});

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: T::BlockNumber) {
			Self::process_rental_periods().unwrap();
		}
	}
}
