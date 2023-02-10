use frame_support::assert_noop;

use crate::{
	mock::{ExtBuilder, NftOnRent, RuntimeEvent, RuntimeOrigin, System, Test},
	CollectibleMap, Error, Event, LesseeCollectiblesDoubleMap,
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

		System::assert_has_event(RuntimeEvent::NftOnRent(Event::RentalPeriodAdded {
			collectible_id: COLLECTIBLE_ID,
			next_rent_block: 11,
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
