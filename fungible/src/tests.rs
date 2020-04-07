use crate::{ mock::* };
use frame_support::{ assert_ok };

#[test]
fn it_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(0,0);
    });
}

#[test]
fn it_creates_a_token() {
    new_test_ext().execute_with(|| {
        assert_eq!(FungiblePallet::token_count(), 0);
        assert_eq!(FungiblePallet::create_token(1, 42), 0u64.into());
        assert_eq!(FungiblePallet::token_count(), 1);
        assert_eq!(
            FungiblePallet::balance_of((0, 1)),
            42
        );
        assert_eq!(
            FungiblePallet::total_supply(0),
            42
        );
    });
}

#[test]
fn it_transfers_a_token() {
    new_test_ext().execute_with(|| {
        assert_eq!(FungiblePallet::create_token(1, 42), 0u64.into());
        assert_ok!(
            FungiblePallet::transfer(Origin::signed(1), 0, 2, 22)
        );
        // Check that each account has correct balances.
        assert_eq!(
            FungiblePallet::balance_of((0, 1)),
            20,
        );
        assert_eq!(FungiblePallet::balance_of((0,2)), 22);
    });
}

#[test]
fn it_creates_allowance_and_transfers() {
    new_test_ext().execute_with(|| {
        assert_eq!(FungiblePallet::create_token(1, 42), 0u64.into());
        assert_ok!(
            FungiblePallet::approve(Origin::signed(1), 0, 2, 20)
        );
        assert_eq!(FungiblePallet::allowance((0, 1, 2)), 20);
        assert_ok!(
            FungiblePallet::transfer_from(Origin::signed(2), 0, 1, 3, 10)
        );
        assert_eq!(FungiblePallet::allowance((0, 1, 2)), 10);
        assert_eq!(FungiblePallet::balance_of((0, 1)), 32);
        assert_eq!(FungiblePallet::balance_of((0, 3)), 10);
    });
}