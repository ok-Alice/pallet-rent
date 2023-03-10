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
		pub lessee: Option<T::AccountId>,
		pub rentable: bool,
		pub minimum_rental_period: Option<u32>,
		pub maximum_rental_period: Option<u32>,
	}

	/// Maps the Collectible struct to the unique_id.
	#[pallet::storage]
	pub(super) type Collectibles<T: Config> = StorageMap<_, Twox64Concat, [u8; 16], Collectible<T>>;

	/// Maps the account id to the owned collectibles.
	#[pallet::storage]
	pub(super) type LessorCollectibles<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<[u8; 16], T::MaximumOwned>>;

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
		[u8; 16],
		RentalPeriodConfig<T>,
	>;

	/// List of rentable collectibles.
	#[pallet::storage]
	pub(super) type RentableCollectibles<T: Config> =
		StorageValue<_, BoundedVec<[u8; 16], T::MaximumOwned>, ValueQuery>;

	/// Track rental periods.
	#[pallet::storage]
	pub(super) type PendingRentals<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::BlockNumber,
		BoundedVec<([u8; 16], T::AccountId), T::MaximumRentablesPerBlock>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type AccountEquips<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<[u8; 16], T::MaximumOwned>>;

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
		},
		/// A rental period was successfully added.
		RentalPeriodAdded { collectible: [u8; 16], next_rent_block: T::BlockNumber },
		/// A rental period was successfully added.
		RentalPeriodRemoved { collectible: [u8; 16], at_block: T::BlockNumber },
		/// A rental period was successfully ended.
		RentalEnded { lessor: T::AccountId, lessee: T::AccountId, collectible: [u8; 16] },
		/// Collectible rent made unavailable.
		RentMadeUnavailable { collectible: [u8; 16] },
		/// Collectible rent made recurring.
		RentalSetRecurring { collectible: [u8; 16], recurring: bool },
		/// Cancelled recurring rental since payment was not made.
		ErrorTransferingRent { lessor: T::AccountId, lessee: T::AccountId, collectible: [u8; 16] },
		/// Collectible equipped by account.
		CollectibleEquipped { account: T::AccountId, collectible: [u8; 16] },
		/// Collectible unequipped by account.
		CollectibleUnequipped { account: T::AccountId, collectible: [u8; 16] },
		/// Rental extended.
		RentalExtended {
			lessor: T::AccountId,
			lessee: T::AccountId,
			collectible: [u8; 16],
			next_rent_block: T::BlockNumber,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Each collectible must have a unique identifier
		DuplicateCollectible,
		/// The collectible doesn't exist
		NoCollectible,
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
			let collectible_gen_unique_id = Self::_gen_unique_id();

			Self::do_mint(&sender, collectible_gen_unique_id)?;

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn burn(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			ensure!(collectible.lessor == sender, Error::<T>::NotLessor);
			ensure!(collectible.lessee == None, Error::<T>::NotAllowedWhileRented);

			Collectibles::<T>::remove(&unique_id);

			let mut lessor_collectibles = LessorCollectibles::<T>::get(&sender).unwrap_or_default();
			lessor_collectibles.retain(|&x| x != unique_id);
			LessorCollectibles::<T>::insert(&sender, lessor_collectibles);

			Self::_unequip_collectible_from_account(sender.clone(), collectible.unique_id);

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(2)]
		pub fn set_rentable(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			price_per_block: BalanceOf<T>,
			minimum_rental_period: u32,
			maximum_rental_period: u32,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessor == sender, Error::<T>::NotLessor);
			ensure!(
				minimum_rental_period <= maximum_rental_period,
				Error::<T>::MinimumMustBeLessThanMaximum
			);
			ensure!(collectible.lessee == None, Error::<T>::NotAllowedWhileRented);

			collectible.price_per_block = Some(price_per_block);
			collectible.rentable = true;
			collectible.minimum_rental_period = Some(minimum_rental_period);
			collectible.maximum_rental_period = Some(maximum_rental_period);

			Collectibles::<T>::insert(&unique_id, &collectible);

			let mut rentable_collectibles = RentableCollectibles::<T>::get();

			rentable_collectibles
				.try_push(unique_id)
				.map_err(|_| Error::<T>::TooManyCollectibles)?;

			RentableCollectibles::<T>::put(rentable_collectibles);

			Self::_unequip_collectible_from_account(sender.clone(), collectible.unique_id);

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

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			if let Some(minimum_rental_period) = collectible.minimum_rental_period {
				ensure!(blocks >= minimum_rental_period, Error::<T>::RentalPeriodTooShort);
			}

			if let Some(maximum_rental_period) = collectible.maximum_rental_period {
				ensure!(blocks <= maximum_rental_period, Error::<T>::RentalPeriodTooLong);
			}

			ensure!(collectible.lessor != lessee, Error::<T>::CannotRentOwnCollectible);
			ensure!(collectible.lessee.is_none(), Error::<T>::RentNotAvailable);
			ensure!(collectible.lessee != Some(lessee.clone()), Error::<T>::AlreadyRented);

			Self::do_rent_collectible(unique_id, lessee, blocks, recurring)?;
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(4)]
		pub fn set_unrentable(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let mut collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessor == sender, Error::<T>::NotLessor);

			collectible.rentable = false;

			Collectibles::<T>::insert(&unique_id, collectible);

			let mut rentable_collectibles = RentableCollectibles::<T>::get();
			rentable_collectibles.retain(|&x| x != unique_id);
			RentableCollectibles::<T>::put(rentable_collectibles);

			Self::deposit_event(Event::RentMadeUnavailable { collectible: unique_id });
			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(5)]
		pub fn set_recurring(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			recurring: bool,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessee == Some(sender.clone()), Error::<T>::NotLessor);

			let lessee_rental = LesseeCollectibles::<T>::get(&sender, &unique_id);

			ensure!(lessee_rental.is_some(), Error::<T>::NoCollectible);

			let mut lessee_rental = lessee_rental.unwrap();

			lessee_rental.recurring = recurring;

			LesseeCollectibles::<T>::insert(sender, &unique_id, lessee_rental);

			Collectibles::<T>::insert(&unique_id, collectible);

			Self::deposit_event(Event::RentalSetRecurring { collectible: unique_id, recurring });

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(6)]
		pub fn extend_rent(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			blocks: T::BlockNumber,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;
			ensure!(collectible.lessee == Some(sender.clone()), Error::<T>::NotLessor);

			Self::do_extend_rent(collectible, blocks)?;

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(7)]
		pub fn equip_collectible(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let account: T::AccountId;

			if collectible.lessor == sender {
				ensure!(!collectible.rentable, Error::<T>::NotAllowedWhileRented);
				account = collectible.lessor;
			} else if let Some(lessee) = collectible.lessee {
				ensure!(lessee == sender, Error::<T>::NotLessee);
				account = lessee;
			} else {
				return Err(Error::<T>::NoAccountFoundForCollectible.into())
			};

			let mut vec = AccountEquips::<T>::get(&account).unwrap_or_default();
			vec.try_push(unique_id.clone())
				.map_err(|_| Error::<T>::TooManyCollectiblesEquiped)?;
			AccountEquips::<T>::insert(&account, vec);

			Self::deposit_event(Event::CollectibleEquipped {
				account: sender,
				collectible: unique_id,
			});

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(8)]
		pub fn unequip_collectible(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collectible =
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			Self::_unequip_collectible_from_account(sender.clone(), collectible.unique_id);

			Self::deposit_event(Event::CollectibleUnequipped {
				account: sender,
				collectible: unique_id,
			});

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
				lessee: None,
				rentable: false,
				minimum_rental_period: None,
				maximum_rental_period: None,
			};

			ensure!(
				!Collectibles::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateCollectible
			);

			Collectibles::<T>::insert(collectible.unique_id, &collectible);

			let mut lessor_collectibles = LessorCollectibles::<T>::get(&lessor).unwrap_or_default();
			lessor_collectibles
				.try_push(collectible.unique_id)
				.map_err(|_| Error::<T>::TooManyCollectiblesOwned)?;
			LessorCollectibles::<T>::insert(&lessor, lessor_collectibles);

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
				Collectibles::<T>::get(&unique_id).ok_or(Error::<T>::NoCollectible)?;

			let lessor = &collectible.lessor;
			let lessee = lessee.clone();

			let total_rent_price =
				collectible.price_per_block.unwrap() * rent_periodic_interval.into();

			// check if lessee has enough balance to pay for the rental
			ensure!(
				T::Currency::free_balance(&lessee) >= total_rent_price.into(),
				Error::<T>::NotEnoughBalance
			);

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
			});

			collectible.lessee = Some(lessee.clone());

			let next_rent_block = Self::_append_pending_rental_to_available_block(
				None,
				rent_periodic_interval,
				unique_id,
				&lessee,
			)
			.unwrap();

			let rental_config = RentalPeriodConfig {
				rental_periodic_interval: rent_periodic_interval.into(),
				next_rent_block,
				recurring,
			};

			// overwrite rental configuration for the collectible
			LesseeCollectibles::<T>::insert(&lessee, &unique_id, &rental_config);

			Collectibles::<T>::insert(&unique_id, &collectible);

			Ok(())
		}

		fn do_extend_rent(collectible: Collectible<T>, blocks: T::BlockNumber) -> DispatchResult {
			let lessee = collectible.lessee.as_ref().unwrap();

			let lessee_rental = LesseeCollectibles::<T>::get(&lessee, &collectible.unique_id)
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

			// check if lessee has enough balance to pay for the rental
			ensure!(
				T::Currency::free_balance(&lessee) >= total_rent_price.into(),
				Error::<T>::NotEnoughBalance
			);

			// transfer rent price to lessor
			T::Currency::transfer(
				&lessee,
				&collectible.lessor,
				total_rent_price.into(),
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;

			Self::deposit_event(Event::RentPayed {
				lessee: lessee.clone(),
				lessor: collectible.lessor.clone(),
				collectible: collectible.unique_id,
				total_rent_price: total_rent_price.into(),
			});

			let next_rent_block = lessee_rental.next_rent_block;

			// Remove old rental from pending rentals since we are extending it
			let mut pending_rental = PendingRentals::<T>::get(&next_rent_block);
			pending_rental.retain(|(id, _)| *id != collectible.unique_id);
			PendingRentals::<T>::insert(&next_rent_block, &pending_rental);

			Self::deposit_event(Event::RentalPeriodRemoved {
				collectible: collectible.unique_id,
				at_block: next_rent_block,
			});

			// Add new rental to pending rentals to extend the rent
			let next_rent_block = Self::_append_pending_rental_to_available_block(
				Some(next_rent_block),
				convert_to_primitive::<T::BlockNumber, u32>(blocks).unwrap(),
				collectible.unique_id,
				&lessee,
			)?;

			// overwrite rental configuration for the collectible
			let rental_config = RentalPeriodConfig { next_rent_block, ..lessee_rental };
			LesseeCollectibles::<T>::insert(lessee, &collectible.unique_id, &rental_config);

			Self::deposit_event(Event::RentalExtended {
				lessor: collectible.lessor,
				lessee: lessee.clone(),
				collectible: collectible.unique_id,
				next_rent_block,
			});

			Self::deposit_event(Event::RentalPeriodAdded {
				collectible: collectible.unique_id,
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
					Self::_remove_lessee_from_collectible(&lessee, &mut collectible).unwrap();

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
				if let Err(_) = T::Currency::transfer(
					&lessee,
					&collectible.lessor,
					total_rent_price,
					frame_support::traits::ExistenceRequirement::KeepAlive,
				) {
					Self::_remove_lessee_from_collectible(&lessee, &mut collectible).unwrap();

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
					let next_rent_block = Self::_append_pending_rental_to_available_block(
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

		fn _append_pending_rental_to_available_block(
			starting_block_number: Option<T::BlockNumber>,
			additional_rental_blocks: u32,
			collectible_id: [u8; 16],
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

		fn _remove_lessee_from_collectible(
			lessee: &T::AccountId,
			collectible: &mut Collectible<T>,
		) -> DispatchResult {
			let collectible_id = collectible.unique_id;
			LesseeCollectibles::<T>::remove(&lessee, &collectible_id);

			collectible.lessee = None;
			Collectibles::<T>::insert(&collectible_id, collectible);

			Self::_unequip_collectible_from_account(lessee.clone(), collectible_id);

			Ok(())
		}

		fn _unequip_collectible_from_account(account: T::AccountId, collectible_id: [u8; 16]) {
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
