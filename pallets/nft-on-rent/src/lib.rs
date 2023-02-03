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

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum Color {
		Red,
		Yellow,
		Blue,
		Green,
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
		pub color: Color,
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
		// A collectible was successfully sold.
		Rented {
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
		/// Trying to transfer a collectible to yourself
		TransferToSelf,
		/// The bid is lower than the asking price.
		BidPriceTooLow,
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
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			// Generate the unique_id and color using a helper function
			let (collectible_gen_unique_id, color) = Self::gen_unique_id();

			// Write new collectible to storage by calling helper function
			Self::mint(&sender, collectible_gen_unique_id, color, price)?;

			Ok(())
		}

		/// Transfer a collectible to another account.
		/// Any account that holds a collectible can send it to another account.
		/// Transfer resets the price of the collectible, marking it not for sale.
		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			unique_id: [u8; 16],
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let from = ensure_signed(origin)?;
			let collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.owner == from, Error::<T>::NotOwner);
			Self::do_transfer(unique_id, to)?;
			Ok(())
		}

		/// Update the collectible price and write to storage.
		#[pallet::weight(0)]
		#[pallet::call_index(2)]
		pub fn set_price(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;
			// Ensure the collectible exists and is called by the owner
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.owner == sender, Error::<T>::NotOwner);
			// Set the price in storage
			collectible.price = new_price;
			CollectibleMap::<T>::insert(&unique_id, collectible);

			// Deposit a "PriceSet" event.
			Self::deposit_event(Event::PriceSet { collectible: unique_id, price: new_price });
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn rent_collectible(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let renter = ensure_signed(origin)?;
			// Transfer the collectible from seller to buyer.
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
			color: Color,
			price: Option<BalanceOf<T>>,
		) -> Result<[u8; 16], DispatchError> {
			// Create a new object
			let collectible =
				Collectible::<T> { unique_id, price, color, owner: owner.clone(), renter: None };

			// Check if the collectible exists in the storage map
			ensure!(
				!CollectibleMap::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateCollectible
			);
			// Append collectible to OwnerOfCollectibles map
			OwnerOfCollectibles::<T>::try_append(owner, collectible.unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;

			// Write new collectible to storage and update the count
			CollectibleMap::<T>::insert(collectible.unique_id, collectible);

			// Deposit the "CollectibleCreated" event.
			Self::deposit_event(Event::CollectibleCreated {
				collectible: unique_id,
				owner: owner.clone(),
			});

			// Returns the unique_id of the new collectible if this succeeds
			Ok(unique_id)
		}

		// Generates and returns the unique_id and color
		fn gen_unique_id() -> ([u8; 16], Color) {
			// Create randomness
			let random = T::CollectionRandomness::random(&b"unique_id"[..]).0;

			// Create randomness payload. Multiple collectibles can be generated in the same block,
			// retaining uniqueness.
			let unique_payload = (
				random,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			// Turns into a byte array
			let encoded_payload = unique_payload.encode();
			let hash = frame_support::Hashable::blake2_128(&encoded_payload);

			// Generate Color
			if hash[0] % 2 == 0 {
				(hash, Color::Red)
			} else {
				(hash, Color::Yellow)
			}
		}

		// Update storage to transfer collectible
		pub fn do_transfer(collectible_id: [u8; 16], to: T::AccountId) -> DispatchResult {
			// Get the collectible
			let mut collectible =
				CollectibleMap::<T>::get(&collectible_id).ok_or(Error::<T>::NoCollectible)?;
			let from = collectible.owner;

			ensure!(from != to, Error::<T>::TransferToSelf);
			let mut from_owned = OwnerOfCollectibles::<T>::get(&from);

			// Remove collectible from list of owned collectible.
			if let Some(ind) = from_owned.iter().position(|&id| id == collectible_id) {
				from_owned.swap_remove(ind);
			} else {
				return Err(Error::<T>::NoCollectible.into())
			}
			// Add collectible to the list of owned collectibles.
			let mut to_owned = OwnerOfCollectibles::<T>::get(&to);
			to_owned.try_push(collectible_id).map_err(|_| Error::<T>::TooManyCollectibles)?;

			// Transfer succeeded, update the owner and reset the price to `None`.
			collectible.owner = to.clone();
			collectible.price = None;

			// Write updates to storage
			CollectibleMap::<T>::insert(&collectible_id, collectible);
			OwnerOfCollectibles::<T>::insert(&to, to_owned);
			OwnerOfCollectibles::<T>::insert(&from, from_owned);

			Self::deposit_event(Event::TransferSucceeded { from, to, collectible: collectible_id });
			Ok(())
		}

		pub fn do_rent_collectible(unique_id: [u8; 16], renter: T::AccountId) -> DispatchResult {
			// Get the collectible from the storage map
			let mut collectible =
				CollectibleMap::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let owner = &collectible.owner;
			let renter = renter.clone();

			// Mutating state with a balance transfer, so nothing is allowed to fail after this.
			if let Some(price) = collectible.price {
				// Transfer the amount from buyer to seller
				T::Currency::transfer(
					&renter,
					&owner,
					price,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				)?;
				// Deposit sold event
				Self::deposit_event(Event::Rented {
					renter: renter.clone(),
					owner: owner.clone(),
					collectible: unique_id,
					price,
				});
			} else {
				return Err(Error::<T>::NotForRent.into())
			}

			let now = <timestamp::Pallet<T>>::get();
			let next_rental_period: T::Moment = now + T::Moment::from(15u32);

			// Set new renter for collectible
			collectible.renter = Some(renter.clone());

			CollectibleMap::<T>::insert(&unique_id, collectible);
			RentalPeriods::<T>::insert(next_rental_period, &unique_id);

			Ok(())
		}

		pub fn process_rental_periods() -> DispatchResult {
			let now = <timestamp::Pallet<T>>::get();
			let rental_periods =
				RentalPeriods::<T>::iter().filter(|(k, _)| *k <= now).collect::<Vec<_>>();

			// for each rental_period, transfer the price amount from renter to owner
			// remove the rental_period from the storage map
			// then get the next Moment for the next rental period
			// and insert the collectible into the storage map with the new rental_period
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

				let next_rental_period: T::Moment = now + T::Moment::from(15u32);
				RentalPeriods::<T>::insert(next_rental_period, v);
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
