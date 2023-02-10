use crate::{
	mock::{ExtBuilder, NftOnRent, RuntimeEvent, RuntimeOrigin, System, Test},
	CollectibleMap, Event,
};

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
		let unique_id: [u8; 16] = [1; 16];

		CollectibleMap::<Test>::insert(
			unique_id,
			crate::Collectible {
				unique_id,
				lessor: 1,
				lessee: None,
				rentable: false,
				price_per_block: None,
				minimum_rental_period: None,
				maximum_rental_period: None,
			},
		);

		NftOnRent::set_rentable(RuntimeOrigin::signed(1), unique_id, 100, 10, 30).unwrap();

		assert_eq!(
			CollectibleMap::<Test>::get(unique_id).unwrap(),
			crate::Collectible {
				unique_id,
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
