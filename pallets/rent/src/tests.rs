use frame_support::{assert_noop, assert_ok};

use crate::{
	mock::{self, run_to_block, ExtBuilder, Rent, RuntimeEvent, RuntimeOrigin, System, Test},
	AccountEquips, Collectibles, Error, Event, LesseeCollectibles, LessorCollectibles,
	PendingRentals, RentableCollectibles,
};

const COLLECTIBLE_ID: [u8; 16] = [1; 16];

#[test]
fn test_mint() {
	ExtBuilder::default().build_and_execute(|| {
		Rent::mint(RuntimeOrigin::signed(1)).unwrap();

		let event = System::events().into_iter().last().unwrap();

		match event.event {
			RuntimeEvent::Rent(Event::CollectibleCreated {
				collectible,
				lessor: _lessor,
				current_lessee: _current_lessee,
			}) => {
				assert!(Collectibles::<Test>::contains_key(collectible));

				let collectible = Collectibles::<Test>::get(collectible).unwrap();

				assert_eq!(collectible.lessor, 1);
				assert_eq!(collectible.lessee, None);
				assert_eq!(collectible.rentable, false);
				assert_eq!(collectible.minimum_rental_period, None);
				assert_eq!(collectible.maximum_rental_period, None);

				let lessor_collectibles = LessorCollectibles::<Test>::get(1).unwrap();

				assert_eq!(lessor_collectibles.len(), 1);
				assert_eq!(lessor_collectibles[0], collectible.collectible_id);
			},
			_ => panic!("Unexpected event"),
		};
	});
}

#[test]
fn test_burn() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, false, None, None, None);

		Rent::burn(RuntimeOrigin::signed(1), COLLECTIBLE_ID).unwrap();

		assert!(!Collectibles::<Test>::contains_key(COLLECTIBLE_ID));

		let lessor_collectibles = LessorCollectibles::<Test>::get(1).unwrap();

		assert!(lessor_collectibles.is_empty());

		let equips = AccountEquips::<Test>::get(1).unwrap_or_default();
		assert!(equips.is_empty());
	});
}

#[test]
fn test_burn_should_fail_if_not_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, false, None, None, None);

		assert_noop!(
			Rent::burn(RuntimeOrigin::signed(2), COLLECTIBLE_ID),
			Error::<Test>::NotLessor
		);
	});
}

#[test]
fn test_burn_should_fail_if_collectible_does_not_exist() {
	ExtBuilder::default().build_and_execute(|| {
		assert_noop!(
			Rent::burn(RuntimeOrigin::signed(1), COLLECTIBLE_ID),
			Error::<Test>::NoCollectible
		);
	});
}

#[test]
fn test_burn_should_fail_if_collectible_has_lessee() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, Some(2), false, None, None, None);

		assert_noop!(
			Rent::burn(RuntimeOrigin::signed(1), COLLECTIBLE_ID),
			Error::<Test>::NotAllowedWhileRented
		);
	});
}

#[test]
fn test_set_rentable() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, false, None, None, None);

		Rent::set_rentable(RuntimeOrigin::signed(1), COLLECTIBLE_ID, 100, 10, 30).unwrap();

		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		assert_eq!(RentableCollectibles::<Test>::get(), vec![COLLECTIBLE_ID])
	});
}

#[test]
fn test_set_rentable_should_fail_if_not_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, false, None, None, None);

		assert_noop!(
			Rent::set_rentable(RuntimeOrigin::signed(2), COLLECTIBLE_ID, 100, 10, 30),
			Error::<Test>::NotLessor
		);
	});
}

#[test]
fn test_set_rentable_should_fail_if_collectible_does_not_exist() {
	ExtBuilder::default().build_and_execute(|| {
		assert_noop!(
			Rent::set_rentable(RuntimeOrigin::signed(1), COLLECTIBLE_ID, 100, 10, 30),
			Error::<Test>::NoCollectible
		);
	});
}

#[test]
fn test_rent_should_fail_if_rental_period_is_too_short() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, true, Some(100), Some(10), Some(30));

		let rent_period: u32 = 5;

		assert_noop!(
			Rent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false),
			Error::<Test>::RentalPeriodTooShort
		);
	});
}

#[test]
fn test_rent_should_fail_if_rental_period_is_too_long() {
	ExtBuilder::default().build_and_execute(|| {
		mock::add_collectible(COLLECTIBLE_ID, 1, None, true, Some(100), Some(10), Some(30));

		let rent_period: u32 = 40;

		assert_noop!(
			Rent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false),
			Error::<Test>::RentalPeriodTooLong
		);
	});
}

#[test]
fn test_rent_non_recurring() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(COLLECTIBLE_ID, 1, None, true, Some(100), Some(10), Some(30));

		let rent_period: u32 = 10;

		Rent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false).unwrap();

		System::assert_has_event(RuntimeEvent::Rent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * rent_period as u64,
		}));

		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		match LesseeCollectibles::<Test>::get(2, COLLECTIBLE_ID) {
			Some(lessee_collectibles) => assert_eq!(
				lessee_collectibles,
				crate::RentalPeriodConfig {
					rental_periodic_interval: 10,
					next_rent_block: 11,
					recurring: false
				}
			),
			None => panic!("No collectible"),
		};
	});
}

#[test]
fn test_pending_rental_process_ending() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: false,
			},
		);

		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(11);

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalEnded {
			lessor: 1,
			lessee: 2,
			collectible: COLLECTIBLE_ID,
		}));

		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		assert_eq!(LesseeCollectibles::<Test>::get(2, COLLECTIBLE_ID), None);

		// Check collectible is no equipped by lessee
		assert_eq!(AccountEquips::<Test>::get(2).unwrap_or_default(), vec![]);
	});
}

#[test]
fn test_pending_rental_process_recurring() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		// Insert lessee collectible with recurring rental
		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: true,
			},
		);

		// Insert pending rental to process in block 11
		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(11);

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalPeriodAdded {
			collectible: COLLECTIBLE_ID,
			next_rent_block: 21,
		}));

		System::assert_has_event(RuntimeEvent::Rent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * 10 as u64,
		}));

		// Check collectible is no longer rented by lessee
		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		// Check lessee no longer has rented collectible
		match LesseeCollectibles::<Test>::get(2, COLLECTIBLE_ID) {
			Some(lessee_collectibles) => assert_eq!(
				lessee_collectibles,
				crate::RentalPeriodConfig {
					rental_periodic_interval: 10,
					next_rent_block: 21,
					recurring: true
				}
			),
			None => panic!("No collectible"),
		};
	});
}

#[test]
fn test_set_recurring_during_ongoing_rental_should_renew_rent() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		// Insert lessee collectible without reccuring rental
		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: false,
			},
		);

		// Insert pending rental to process in block 11
		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(5);

		// before reaching block 11, set recurring rental
		assert_ok!(Rent::set_recurring(RuntimeOrigin::signed(2), COLLECTIBLE_ID, true));

		run_to_block(11);

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalPeriodAdded {
			collectible: COLLECTIBLE_ID,
			next_rent_block: 21,
		}));

		System::assert_has_event(RuntimeEvent::Rent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * 10 as u64,
		}));

		// Check collectible is no longer rented by lessee
		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);
	});
}

#[test]
fn test_set_recurring_during_ongoing_rental_should_not_renew_rent() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		// Insert lessee collectible with reccuring rental
		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: true,
			},
		);

		// Insert pending rental to process in block 11
		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(5);

		// equip collectible
		Rent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		// before reaching block 11, set un-recurring rental
		assert_ok!(Rent::set_recurring(RuntimeOrigin::signed(2), COLLECTIBLE_ID, false));

		run_to_block(11);

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalEnded {
			lessor: 1,
			lessee: 2,
			collectible: COLLECTIBLE_ID,
		}));

		// Check collectible is no longer rented by lessee
		assert_eq!(
			Collectibles::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				collectible_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		// Check collectible is no longer rented by lessee
		assert_eq!(LesseeCollectibles::<Test>::get(2, COLLECTIBLE_ID), None);

		// Check collectible is no equipped by lessee
		assert_eq!(AccountEquips::<Test>::get(2).unwrap_or_default(), vec![]);
	});
}

#[test]
fn test_extend_rent() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		// Insert lessee collectible without reccuring rental
		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: false,
			},
		);

		// Insert pending rental to process in block 11
		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(5);

		// before reaching block 11, extend rental
		assert_ok!(Rent::extend_rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, 3));

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalExtended {
			lessor: 1,
			lessee: 2,
			collectible: COLLECTIBLE_ID,
			next_rent_block: 14,
		}));

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalPeriodAdded {
			collectible: COLLECTIBLE_ID,
			next_rent_block: 14,
		}));

		System::assert_has_event(RuntimeEvent::Rent(Event::RentalPeriodRemoved {
			collectible: COLLECTIBLE_ID,
			at_block: 11,
		}));

		// check that rental period is not present in block 11
		assert_eq!(PendingRentals::<Test>::get(11), vec![]);
		// check that rental period is present in block 14
		assert_eq!(PendingRentals::<Test>::get(14), vec![(COLLECTIBLE_ID, 2)]);
	});
}

#[test]
fn test_extend_rent_should_fail_if_exceeds_maximum_rental_period() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		// Insert lessee collectible without reccuring rental
		LesseeCollectibles::<Test>::insert(
			2,
			COLLECTIBLE_ID,
			crate::RentalPeriodConfig {
				rental_periodic_interval: 10,
				next_rent_block: 11,
				recurring: false,
			},
		);

		// Insert pending rental to process in block 11
		let mut pending_rental = PendingRentals::<Test>::get(11);
		pending_rental.try_append(&mut vec![(COLLECTIBLE_ID, 2)]).unwrap();
		PendingRentals::<Test>::insert(11, pending_rental);

		run_to_block(5);

		// before reaching block 11, extend rental
		assert_noop!(
			Rent::extend_rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, 20),
			Error::<Test>::RentalPeriodTooLong
		);
	});
}

#[test]
fn test_equip_collectible() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		Rent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::Rent(Event::CollectibleEquipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquips::<Test>::get(2).unwrap_or_default(), vec![COLLECTIBLE_ID]);
	});
}

#[test]
fn test_unequip_collectible() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		Rent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::Rent(Event::CollectibleEquipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquips::<Test>::get(2).unwrap_or_default(), vec![COLLECTIBLE_ID]);

		Rent::unequip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::Rent(Event::CollectibleUnequipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquips::<Test>::get(2).unwrap_or_default(), vec![]);
	});
}

#[test]
fn test_should_not_equip_rented_collectible_as_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			Some(2),
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		assert_noop!(
			Rent::equip_collectible(RuntimeOrigin::signed(1), COLLECTIBLE_ID),
			Error::<Test>::NotAllowedWhileRented
		);
	});
}

#[test]
fn test_should_not_equip_rentable_collectible_as_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			None,
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		assert_noop!(
			Rent::equip_collectible(RuntimeOrigin::signed(1), COLLECTIBLE_ID),
			Error::<Test>::NotAllowedWhileRented
		);
	});
}

#[test]
fn test_should_not_equip_unrented_collectible_as_lessee() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		mock::add_collectible(
			COLLECTIBLE_ID,
			1,
			None,
			true,
			Some(price_per_block),
			Some(10),
			Some(30),
		);

		assert_noop!(
			Rent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID),
			Error::<Test>::NoAccountFoundForCollectible
		);
	});
}
