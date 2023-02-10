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
