use frame_support::{assert_noop, assert_ok};

use crate::{
	mock::{run_to_block, ExtBuilder, NftOnRent, RuntimeEvent, RuntimeOrigin, System, Test},
	AccountEquipsMap, CollectibleMap, Error, Event, LesseeCollectiblesDoubleMap, PendingRentals,
};

const COLLECTIBLE_ID: [u8; 16] = [1; 16];

#[test]
fn test_mint() {
	ExtBuilder::default().build_and_execute(|| {
		NftOnRent::mint(RuntimeOrigin::signed(1)).unwrap();

		let event = System::events().into_iter().last().unwrap();

		match event.event {
			RuntimeEvent::NftOnRent(Event::CollectibleCreated {
				collectible,
				lessor: _lessor,
				current_lessee: _current_lessee,
			}) => {
				assert!(CollectibleMap::<Test>::contains_key(collectible));

				let collectible = CollectibleMap::<Test>::get(collectible).unwrap();

				assert_eq!(collectible.lessor, 1);
				assert_eq!(collectible.lessee, None);
				assert_eq!(collectible.rentable, false);
				assert_eq!(collectible.minimum_rental_period, None);
				assert_eq!(collectible.maximum_rental_period, None);
			},
			_ => panic!("Unexpected event"),
		};
	});
}

#[test]
fn test_set_rentable() {
	ExtBuilder::default().build_and_execute(|| {
		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: false,
				price_per_block: None,
				minimum_rental_period: None,
				maximum_rental_period: None,
			},
		);

		NftOnRent::set_rentable(RuntimeOrigin::signed(1), COLLECTIBLE_ID, 100, 10, 30).unwrap();

		assert_eq!(
			CollectibleMap::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);
	});
}

#[test]
fn test_set_rentable_should_fail_if_not_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: false,
				price_per_block: None,
				minimum_rental_period: None,
				maximum_rental_period: None,
			},
		);

		assert_noop!(
			NftOnRent::set_rentable(RuntimeOrigin::signed(2), COLLECTIBLE_ID, 100, 10, 30),
			Error::<Test>::NotLessor
		);
	});
}

#[test]
fn test_set_rentable_should_fail_if_collectible_does_not_exist() {
	ExtBuilder::default().build_and_execute(|| {
		assert_noop!(
			NftOnRent::set_rentable(RuntimeOrigin::signed(1), COLLECTIBLE_ID, 100, 10, 30),
			Error::<Test>::NoCollectible
		);
	});
}

#[test]
fn test_rent_should_fail_if_rental_period_is_too_short() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		let rent_period: u32 = 5;

		assert_noop!(
			NftOnRent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false),
			Error::<Test>::RentalPeriodTooShort
		);
	});
}

#[test]
fn test_rent_should_fail_if_rental_period_is_too_long() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		let rent_period: u32 = 40;

		assert_noop!(
			NftOnRent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false),
			Error::<Test>::RentalPeriodTooLong
		);
	});
}

#[test]
fn test_rent_non_recurring() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		let rent_period: u32 = 10;

		NftOnRent::rent(RuntimeOrigin::signed(2), COLLECTIBLE_ID, rent_period, false).unwrap();

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * rent_period as u64,
		}));

		assert_eq!(
			CollectibleMap::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		match LesseeCollectiblesDoubleMap::<Test>::get(2, COLLECTIBLE_ID) {
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

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		LesseeCollectiblesDoubleMap::<Test>::insert(
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

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentalPeriodEnded {
			lessor: 1,
			lessee: 2,
			collectible: COLLECTIBLE_ID,
		}));

		assert_eq!(
			CollectibleMap::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		assert_eq!(LesseeCollectiblesDoubleMap::<Test>::get(2, COLLECTIBLE_ID), None);
	});
}

#[test]
fn test_pending_rental_process_recurring() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		// Insert lessee collectible with recurring rental
		LesseeCollectiblesDoubleMap::<Test>::insert(
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

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentalPeriodAdded {
			collectible_id: COLLECTIBLE_ID,
			next_rent_block: 21,
		}));

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * 10 as u64,
		}));

		// Check collectible is no longer rented by lessee
		assert_eq!(
			CollectibleMap::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(100),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			}
		);

		// Check lessee no longer has rented collectible
		match LesseeCollectiblesDoubleMap::<Test>::get(2, COLLECTIBLE_ID) {
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
fn test_pending_rental_process_update_recurring() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		// Insert collectible with present lessee
		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		// Insert lessee collectible without reccuring rental
		LesseeCollectiblesDoubleMap::<Test>::insert(
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
		assert_ok!(NftOnRent::set_recurring(RuntimeOrigin::signed(2), COLLECTIBLE_ID, true));

		run_to_block(11);

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentalPeriodAdded {
			collectible_id: COLLECTIBLE_ID,
			next_rent_block: 21,
		}));

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentPayed {
			lessee: 2,
			lessor: 1,
			collectible: COLLECTIBLE_ID,
			total_rent_price: price_per_block * 10 as u64,
		}));

		// Check collectible is no longer rented by lessee
		assert_eq!(
			CollectibleMap::<Test>::get(COLLECTIBLE_ID).unwrap(),
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
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
fn test_equip_collectible() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		NftOnRent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::CollectibleEquipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquipsMap::<Test>::get(2).unwrap_or_default(), vec![COLLECTIBLE_ID]);
	});
}

#[test]
fn test_unequip_collectible() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		NftOnRent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::CollectibleEquipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquipsMap::<Test>::get(2).unwrap_or_default(), vec![COLLECTIBLE_ID]);

		NftOnRent::unequip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID).unwrap();

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::CollectibleUnequipped {
			collectible: COLLECTIBLE_ID,
			account: 2,
		}));

		assert_eq!(AccountEquipsMap::<Test>::get(2).unwrap_or_default(), vec![]);
	});
}

#[test]
fn test_should_not_equip_rented_collectible_as_lessor() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: Some(2),
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		assert_noop!(
			NftOnRent::equip_collectible(RuntimeOrigin::signed(1), COLLECTIBLE_ID),
			Error::<Test>::NotAllowedWhileRented
		);
	});
}

#[test]
fn test_should_not_equip_unrented_collectible_as_lessee() {
	ExtBuilder::default().build_and_execute(|| {
		let price_per_block: u64 = 100;

		CollectibleMap::<Test>::insert(
			COLLECTIBLE_ID,
			crate::Collectible {
				unique_id: COLLECTIBLE_ID,
				lessor: 1,
				lessee: None,
				rentable: true,
				price_per_block: Some(price_per_block),
				minimum_rental_period: Some(10),
				maximum_rental_period: Some(30),
			},
		);

		assert_noop!(
			NftOnRent::equip_collectible(RuntimeOrigin::signed(2), COLLECTIBLE_ID),
			Error::<Test>::NotLessor
		);
	});
}
