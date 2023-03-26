//! # Rent Pallet
//!
//! The Rent pallet provides functionality for renting collectibles.
//!
//! ## Terminology
//!
//! - Lessor: The account that owns a collectible and can rent it.
//! - Lessee: The account that rents a collectible.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod utils;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		ensure,
		pallet_prelude::*,
		traits::{Currency, Get, Randomness},
	};
	use frame_system::pallet_prelude::{OriginFor, *};

	use scale_info::prelude::vec;

	use crate::utils::convert_to_primitive;

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

	type CollectibleId = [u8; 16];

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Collectible<T: Config> {
		// Unsigned integers of 16 bytes to represent a unique identifier
		pub collectible_id: CollectibleId,
		// `None` assumes not for sale
		pub price_per_block: Option<BalanceOf<T>>,
		pub lessor: T::AccountId,
		pub lessee: Option<T::AccountId>,
		pub rentable: bool,
		pub minimum_rental_period: Option<u32>,
		pub maximum_rental_period: Option<u32>,
	}

	/// Maps the Collectible struct to the collectible_id.
	#[pallet::storage]
	pub(super) type Collectibles<T: Config> =
		StorageMap<_, Twox64Concat, CollectibleId, Collectible<T>>;

	/// Maps the account id to the owned collectibles.
	#[pallet::storage]
	pub(super) type LessorCollectibles<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<CollectibleId, T::MaximumOwned>>;

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct RentalPeriodConfig<T: Config> {
		pub rental_periodic_interval: u32,
		pub next_rent_block: T::BlockNumber,
		pub recurring: bool,
	}

	/// Maps the account id to the collectibles rented and the rental configuration.
	#[pallet::storage]
	pub(super) type LesseeCollectibles<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		CollectibleId,
		RentalPeriodConfig<T>,
	>;

	/// List of rentable collectibles.
	#[pallet::storage]
	pub(super) type RentableCollectibles<T: Config> =
		StorageValue<_, BoundedVec<CollectibleId, T::MaximumOwned>, ValueQuery>;

	/// Track rental periods.
	#[pallet::storage]
	pub(super) type PendingRentals<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::BlockNumber,
		BoundedVec<(CollectibleId, T::AccountId), T::MaximumRentablesPerBlock>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type AccountEquips<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<CollectibleId, T::MaximumOwned>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new collectible was successfully created.
		CollectibleCreated {
			collectible: CollectibleId,
			lessor: T::AccountId,
			current_lessee: Option<T::AccountId>,
		},
		/// A collectible was successfully transferred.
		TransferSucceeded { from: T::AccountId, to: T::AccountId, collectible: CollectibleId },
		/// The price of a collectible was successfully set.
		RentMadeAvailable { collectible: CollectibleId, price_per_block: BalanceOf<T> },
		/// A collectible was successfully rented.
		RentPayed {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: CollectibleId,
			total_rent_price: BalanceOf<T>,
		},
		/// A rental period was successfully added.
		RentalPeriodAdded { collectible: CollectibleId, next_rent_block: T::BlockNumber },
		/// A rental period was successfully added.
		RentalPeriodRemoved { collectible: CollectibleId, at_block: T::BlockNumber },
		/// A rental period was successfully ended.
		RentalEnded { lessor: T::AccountId, lessee: T::AccountId, collectible: CollectibleId },
		/// Collectible rent made unavailable.
		RentMadeUnavailable { collectible: CollectibleId },
		/// Collectible rent made recurring.
		RentalSetRecurring { collectible: CollectibleId, recurring: bool },
		/// Cancelled recurring rental since payment was not made.
		ErrorTransferingRent {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: CollectibleId,
		},
		/// Collectible equipped by account.
		CollectibleEquipped { account: T::AccountId, collectible: CollectibleId },
		/// Collectible unequipped by account.
		CollectibleUnequipped { account: T::AccountId, collectible: CollectibleId },
		/// Rental extended.
		RentalExtended {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: CollectibleId,
			next_rent_block: T::BlockNumber,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Each collectible must have a unique identifier
		DuplicateCollectible,
		/// The collectible doesn't exist
		NoCollectible,
		/// No collectible has no lessee
		NoLessee,
		/// You are not the lessor of this collectible.
		NotLessor,
		/// You are not the lessee of this collectible.
		NotLessee,
		/// You are already the lessee of this collectible
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
		/// Lessor cannot perform operation while collectible is rented.
		NotAllowedWhileRented,
		/// Account reached max equiped collectibles.
		TooManyCollectiblesEquiped,
		/// Account reached max owned collectibles.
		TooManyCollectiblesOwned,
		/// Not enough balance to rent collectible.
		NotEnoughBalance,
		/// Minimum must be less or equal than maximum.
		MinimumMustBeLessThanMaximum,
		/// No account found associated with collectible.
		NoAccountFoundForCollectible,
	}

	// Pallet callable functions
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn mint(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let collectible_gen_collectible_id = Self::gen_collectible_id();

			Self::do_mint(&sender, collectible_gen_collectible_id)?;

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn burn(origin: OriginFor<T>, collectible_id: CollectibleId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;

			Self::ensure_user_is_lessor(&sender, &collectible)?;
			ensure!(collectible.lessee == None, Error::<T>::NotAllowedWhileRented);

			Collectibles::<T>::remove(&collectible_id);

			let mut lessor_collectibles = LessorCollectibles::<T>::get(&sender).unwrap_or_default();
			lessor_collectibles.retain(|&x| x != collectible_id);
			LessorCollectibles::<T>::insert(&sender, lessor_collectibles);

			Self::unequip_collectible_from_account(sender.clone(), collectible.collectible_id);

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(2)]
		pub fn set_rentable(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
			price_per_block: BalanceOf<T>,
			minimum_rental_period: u32,
			maximum_rental_period: u32,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible = Self::fetch_collectible(collectible_id)?;
			Self::ensure_user_is_lessor(&sender, &collectible)?;
			ensure!(
				minimum_rental_period <= maximum_rental_period,
				Error::<T>::MinimumMustBeLessThanMaximum
			);
			ensure!(collectible.lessee == None, Error::<T>::NotAllowedWhileRented);

			collectible.price_per_block = Some(price_per_block);
			collectible.rentable = true;
			collectible.minimum_rental_period = Some(minimum_rental_period);
			collectible.maximum_rental_period = Some(maximum_rental_period);

			Collectibles::<T>::insert(&collectible_id, &collectible);

			let mut rentable_collectibles = RentableCollectibles::<T>::get();

			rentable_collectibles
				.try_push(collectible_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;

			RentableCollectibles::<T>::put(rentable_collectibles);

			Self::unequip_collectible_from_account(sender.clone(), collectible.collectible_id);

			Self::deposit_event(Event::RentMadeAvailable {
				collectible: collectible_id,
				price_per_block,
			});
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(3)]
		pub fn rent(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
			blocks: u32,
			recurring: bool,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;

			if let Some(minimum_rental_period) = collectible.minimum_rental_period {
				ensure!(blocks >= minimum_rental_period, Error::<T>::RentalPeriodTooShort);
			}

			if let Some(maximum_rental_period) = collectible.maximum_rental_period {
				ensure!(blocks <= maximum_rental_period, Error::<T>::RentalPeriodTooLong);
			}

			ensure!(collectible.lessor != sender, Error::<T>::CannotRentOwnCollectible);
			ensure!(collectible.lessee.is_none(), Error::<T>::RentNotAvailable);
			ensure!(collectible.lessee != Some(sender.clone()), Error::<T>::AlreadyRented);

			Self::do_rent_collectible(collectible_id, sender, blocks, recurring)?;
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(4)]
		pub fn set_unrentable(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible = Self::fetch_collectible(collectible_id)?;
			Self::ensure_user_is_lessor(&sender, &collectible)?;

			collectible.rentable = false;

			Collectibles::<T>::insert(&collectible_id, collectible);

			let mut rentable_collectibles = RentableCollectibles::<T>::get();
			rentable_collectibles.retain(|&x| x != collectible_id);
			RentableCollectibles::<T>::put(rentable_collectibles);

			Self::deposit_event(Event::RentMadeUnavailable { collectible: collectible_id });
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(5)]
		pub fn set_recurring(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
			recurring: bool,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;
			Self::ensure_user_is_lessee(&sender, &collectible)?;

			let lessee_rental = LesseeCollectibles::<T>::get(&sender, &collectible_id);

			ensure!(lessee_rental.is_some(), Error::<T>::NoCollectible);

			let mut lessee_rental = lessee_rental.unwrap();

			lessee_rental.recurring = recurring;

			LesseeCollectibles::<T>::insert(sender, &collectible_id, lessee_rental);

			Collectibles::<T>::insert(&collectible_id, collectible);

			Self::deposit_event(Event::RentalSetRecurring {
				collectible: collectible_id,
				recurring,
			});

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(6)]
		pub fn extend_rent(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
			blocks: T::BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;
			Self::ensure_user_is_lessee(&sender, &collectible)?;

			Self::do_extend_rent(collectible, blocks)?;

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(7)]
		pub fn equip_collectible(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;

			let account: T::AccountId;

			if Self::ensure_user_is_lessor(&sender, &collectible).ok().is_some() {
				ensure!(!collectible.rentable, Error::<T>::NotAllowedWhileRented);
				account = collectible.lessor;
			} else if let Some(lessee) = collectible.lessee.clone() {
				Self::ensure_user_is_lessee(&sender, &collectible)?;
				account = lessee;
			} else {
				return Err(Error::<T>::NoAccountFoundForCollectible.into())
			};

			let mut vec = AccountEquips::<T>::get(&account).unwrap_or_default();
			vec.try_push(collectible_id.clone())
				.map_err(|_| Error::<T>::TooManyCollectiblesEquiped)?;
			AccountEquips::<T>::insert(&account, vec);

			Self::deposit_event(Event::CollectibleEquipped {
				account: sender,
				collectible: collectible_id,
			});

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(8)]
		pub fn unequip_collectible(
			origin: OriginFor<T>,
			collectible_id: CollectibleId,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible = Self::fetch_collectible(collectible_id)?;
			if !Self::ensure_user_is_lessee(&sender, &collectible).ok().is_some() {
				Self::ensure_user_is_lessor(&sender, &collectible)?;
			}

			Self::unequip_collectible_from_account(sender.clone(), collectible.collectible_id);

			Self::deposit_event(Event::CollectibleUnequipped {
				account: sender,
				collectible: collectible_id,
			});

			Ok(())
		}
	}

	// Pallet internal functions
	impl<T: Config> Pallet<T> {
		// Function to mint a collectible
		pub fn do_mint(
			lessor: &T::AccountId,
			collectible_id: CollectibleId,
		) -> Result<CollectibleId, DispatchError> {
			let collectible = Collectible::<T> {
				collectible_id,
				price_per_block: None,
				lessor: lessor.clone(),
				lessee: None,
				rentable: false,
				minimum_rental_period: None,
				maximum_rental_period: None,
			};

			ensure!(
				!Collectibles::<T>::contains_key(&collectible.collectible_id),
				Error::<T>::DuplicateCollectible
			);

			Collectibles::<T>::insert(collectible.collectible_id, &collectible);

			let mut lessor_collectibles = LessorCollectibles::<T>::get(&lessor).unwrap_or_default();
			lessor_collectibles
				.try_push(collectible.collectible_id)
				.map_err(|_| Error::<T>::TooManyCollectiblesOwned)?;
			LessorCollectibles::<T>::insert(&lessor, lessor_collectibles);

			Self::deposit_event(Event::CollectibleCreated {
				collectible: collectible_id,
				lessor: lessor.clone(),
				current_lessee: None,
			});

			Ok(collectible_id)
		}

		fn do_rent_collectible(
			collectible_id: CollectibleId,
			lessee: T::AccountId,
			rent_periodic_interval: u32,
			recurring: bool,
		) -> DispatchResult {
			let mut collectible = Self::fetch_collectible(collectible_id)?;

			let lessor = &collectible.lessor;
			let lessee = lessee.clone();

			let total_rent_price =
				collectible.price_per_block.unwrap() * rent_periodic_interval.into();

			Self::transfer_funds(&lessee, &lessor, total_rent_price)?;

			Self::deposit_event(Event::RentPayed {
				lessee: lessee.clone(),
				lessor: lessor.clone(),
				collectible: collectible_id,
				total_rent_price,
			});

			collectible.lessee = Some(lessee.clone());

			let next_rent_block = Self::append_pending_rental_to_available_block(
				None,
				rent_periodic_interval,
				collectible_id,
				&lessee,
			)
			.unwrap();

			let rental_config = RentalPeriodConfig {
				rental_periodic_interval: rent_periodic_interval.into(),
				next_rent_block,
				recurring,
			};

			// overwrite rental configuration for the collectible
			LesseeCollectibles::<T>::insert(&lessee, &collectible_id, &rental_config);

			Collectibles::<T>::insert(&collectible_id, &collectible);

			Ok(())
		}

		fn do_extend_rent(collectible: Collectible<T>, blocks: T::BlockNumber) -> DispatchResult {
			let lessee = collectible.lessee.as_ref().unwrap();

			let lessee_rental = LesseeCollectibles::<T>::get(&lessee, &collectible.collectible_id)
				.ok_or(Error::<T>::NoCollectible)?;

			ensure!(
				blocks + lessee_rental.next_rent_block <=
					collectible.maximum_rental_period.unwrap().into(),
				Error::<T>::RentalPeriodTooLong
			);

			// add blocks and rental period interval
			let total_rent_price = convert_to_primitive::<T::BlockNumber, u32>(blocks).unwrap() *
				convert_to_primitive::<BalanceOf<T>, u32>(collectible.price_per_block.unwrap())
					.unwrap();

			Self::transfer_funds(&lessee, &collectible.lessor, total_rent_price.into())?;

			Self::deposit_event(Event::RentPayed {
				lessee: lessee.clone(),
				lessor: collectible.lessor.clone(),
				collectible: collectible.collectible_id,
				total_rent_price: total_rent_price.into(),
			});

			let next_rent_block = lessee_rental.next_rent_block;

			// Remove old rental from pending rentals since we are extending it
			let mut pending_rental = PendingRentals::<T>::get(&next_rent_block);
			pending_rental.retain(|(id, _)| *id != collectible.collectible_id);
			PendingRentals::<T>::insert(&next_rent_block, &pending_rental);

			Self::deposit_event(Event::RentalPeriodRemoved {
				collectible: collectible.collectible_id,
				at_block: next_rent_block,
			});

			// Add new rental to pending rentals to extend the rent
			let next_rent_block = Self::append_pending_rental_to_available_block(
				Some(next_rent_block),
				convert_to_primitive::<T::BlockNumber, u32>(blocks).unwrap(),
				collectible.collectible_id,
				&lessee,
			)?;

			// overwrite rental configuration for the collectible
			let rental_config = RentalPeriodConfig { next_rent_block, ..lessee_rental };
			LesseeCollectibles::<T>::insert(lessee, &collectible.collectible_id, &rental_config);

			Self::deposit_event(Event::RentalExtended {
				lessor: collectible.lessor,
				lessee: lessee.clone(),
				collectible: collectible.collectible_id,
				next_rent_block,
			});

			Self::deposit_event(Event::RentalPeriodAdded {
				collectible: collectible.collectible_id,
				next_rent_block,
			});

			Ok(())
		}

		fn do_process_rental_periods(n: T::BlockNumber) {
			let rentals = PendingRentals::<T>::get(n);

			for rental in rentals {
				let collectible_id = rental.0.clone();
				let lessee = rental.1.clone();

				let rental_config = LesseeCollectibles::<T>::get(&lessee, &collectible_id).unwrap();

				let mut collectible = Collectibles::<T>::get(&collectible_id)
					.ok_or(Error::<T>::NoCollectible)
					.unwrap();

				if !collectible.rentable || None == collectible.lessee || !rental_config.recurring {
					Self::remove_lessee_from_collectible(&lessee, &mut collectible).unwrap();

					Self::deposit_event(Event::RentalEnded {
						lessee: lessee.clone(),
						lessor: collectible.lessor.clone(),
						collectible: collectible_id,
					});

					continue
				}

				let total_rent_price = collectible.price_per_block.unwrap() *
					rental_config.rental_periodic_interval.into();

				// Mutating state with a balance transfer, so nothing is allowed to fail after
				// this.
				if let Err(_) = Self::transfer_funds(&lessee, &collectible.lessor, total_rent_price)
				{
					Self::remove_lessee_from_collectible(&lessee, &mut collectible).unwrap();

					Self::deposit_event(Event::ErrorTransferingRent {
						lessee: lessee.clone(),
						lessor: collectible.lessor.clone(),
						collectible: collectible_id,
					});

					continue
				}

				Self::deposit_event(Event::RentPayed {
					lessee: lessee.clone(),
					lessor: collectible.lessor.clone(),
					collectible: collectible_id,
					total_rent_price,
				});

				// Add the rental period again if recurring
				if rental_config.recurring {
					let next_rent_block = Self::append_pending_rental_to_available_block(
						None,
						rental_config.rental_periodic_interval,
						collectible_id,
						&lessee,
					)
					.unwrap();

					LesseeCollectibles::<T>::insert(
						&lessee,
						&collectible_id,
						RentalPeriodConfig { next_rent_block, ..rental_config },
					);
				}
			}

			PendingRentals::<T>::remove(n);
		}
	}

	// Pallet helper functions
	impl<T: Config> Pallet<T> {
		// Generates and returns the collectible_id
		fn gen_collectible_id() -> CollectibleId {
			let random = T::CollectionRandomness::random(&b"collectible_id"[..]).0;

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

		fn append_pending_rental_to_available_block(
			starting_block_number: Option<T::BlockNumber>,
			additional_rental_blocks: u32,
			collectible_id: CollectibleId,
			lessee: &T::AccountId,
		) -> Result<T::BlockNumber, DispatchError> {
			let starting_block_number = match starting_block_number {
				Some(block_number) => block_number,
				None => frame_system::Pallet::<T>::block_number(),
			};

			let mut block_number = starting_block_number + additional_rental_blocks.into();

			let mut rental_periods = PendingRentals::<T>::get(block_number);

			// try to append the collectible to the rental period
			// if it fails (because the rental period is already full), increment the rental period
			// and try again
			while let Err(_) =
				rental_periods.try_append(&mut vec![(collectible_id, lessee.clone())])
			{
				let next_block_number: T::BlockNumber = (1 as u32).into();
				block_number = frame_system::Pallet::<T>::block_number() + next_block_number;
				rental_periods = PendingRentals::<T>::get(block_number);
			}

			PendingRentals::<T>::insert(block_number, rental_periods);

			Self::deposit_event(Event::RentalPeriodAdded {
				collectible: collectible_id,
				next_rent_block: block_number,
			});

			Ok(block_number)
		}

		fn remove_lessee_from_collectible(
			lessee: &T::AccountId,
			collectible: &mut Collectible<T>,
		) -> DispatchResult {
			let collectible_id = collectible.collectible_id;
			LesseeCollectibles::<T>::remove(&lessee, &collectible_id);

			collectible.lessee = None;
			Collectibles::<T>::insert(&collectible_id, collectible);

			Self::unequip_collectible_from_account(lessee.clone(), collectible_id);

			Ok(())
		}

		fn unequip_collectible_from_account(account: T::AccountId, collectible_id: CollectibleId) {
			let mut equiped = AccountEquips::<T>::get(&account).unwrap_or_default();
			let initial_size = equiped.len().clone();
			equiped.retain(|c| c != &collectible_id);
			AccountEquips::<T>::insert(&account, &equiped);

			if initial_size != equiped.len() {
				Self::deposit_event(Event::CollectibleUnequipped {
					account,
					collectible: collectible_id,
				});
			}
		}

		fn fetch_collectible(
			collectible_id: CollectibleId,
		) -> Result<Collectible<T>, DispatchError> {
			let collectible = Collectibles::<T>::try_get(&collectible_id)
				.map_err(|_| Error::<T>::NoCollectible)?;

			Ok(collectible)
		}

		fn ensure_user_is_lessee(
			user: &T::AccountId,
			collectible: &Collectible<T>,
		) -> Result<(), Error<T>> {
			if collectible.lessee.is_none() {
				return Err(Error::<T>::NoLessee)
			}

			if collectible.lessee.as_ref().unwrap() != user {
				return Err(Error::<T>::NotLessee)
			}

			Ok(())
		}

		fn ensure_user_is_lessor(
			user: &T::AccountId,
			collectible: &Collectible<T>,
		) -> Result<(), Error<T>> {
			if collectible.lessor != *user {
				return Err(Error::<T>::NotLessor)
			}

			Ok(())
		}

		fn transfer_funds(
			from: &T::AccountId,
			to: &T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			// check if lessee has enough balance to pay for the rental
			ensure!(T::Currency::free_balance(from) >= amount, Error::<T>::NotEnoughBalance);

			T::Currency::transfer(
				from,
				to,
				amount,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::do_process_rental_periods(n);

			// TODO: Calculate weight
			Weight::from_parts(0, 0)
		}
	}
}
