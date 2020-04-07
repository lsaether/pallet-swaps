use crate::{Error, mock::*};
use frame_support::{assert_ok, assert_noop};

#[test]
fn creates_a_new_swap() {
	new_test_ext().execute_with(|| {
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));
		assert_eq!(Fungible::token_count(), 1);

		assert_eq!(Swaps::swap_count(), 0);
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));
		assert_eq!(Swaps::swap_count(), 1);
		assert_eq!(Fungible::token_count(), 2);
		let swap_id = Swaps::token_to_swap(0);
		assert_eq!(swap_id, 0);
		let swap = Swaps::swaps(0).unwrap();
		assert_eq!(swap.token_id, 0);
		assert_eq!(swap.swap_token, 1);
		assert_eq!(swap.account, 3415826855702589293);
	});
}

#[test]
fn cannot_create_a_second_swap_for_identical_token() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));
		
		// Create SwapId 0 for TokenId 0.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Fails creating a second swap for TokenId 0.
		assert_noop!(Swaps::create_swap(Origin::signed(1), 0), Error::<Test>::SwapAlreadyExists);
	});
}

#[test]
fn can_add_liquidity_when_total_liquidity_is_zero() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		 assert_ok!(
			 Swaps::add_liquidity(
				 Origin::signed(1),
				 0,
				 420,
				 0,
				 42,
				 100,
			 )
		 );

		 let swap = Swaps::swaps(0).unwrap();

		 // Balance left the sender...
		 let sender_bal = Balances::free_balance(&1);
		 assert_eq!(sender_bal, 10000 - 420);
		 // ... and went into the swap account.
		 let swap_bal = Balances::free_balance(&swap.account);
		 assert_eq!(swap_bal, 420);

		 // TokenId 0 left the sender...
		 let sender_tokens = Fungible::balance_of((0, 1));
		 assert_eq!(sender_tokens, 0);
		 // .. and went into the swap account.
		 let swap_tokens = Fungible::balance_of((0, swap.account));
		 assert_eq!(swap_tokens, 42);

		 // TokenId exists in sender's account.
		 let sender_token_ones = Fungible::balance_of((1, 1));
		 assert_eq!(sender_token_ones, 420);
	});
}

#[test]
fn it_errors_when_trying_to_add_liqudity_to_nonexistent_swap() {
	new_test_ext().execute_with(|| {
		// Fails to add liquidity to SwapId 33 (doesn't exist).
		assert_noop!(
			Swaps::add_liquidity(
				Origin::signed(1),
				33,
				420,
				0,
				42,
				100,
			),
			Error::<Test>::NoSwapExists,
		);
	});
}

#[test]
fn it_adds_liquidity_to_swap_with_liquidity() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				200,
				0,
				20,
				100,
			)
		);

		// First check when asking for more than enough liqudiity.
		assert_noop!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				100,
				101, // too high
				10,
				100,
			),
			Error::<Test>::TooLowLiquidity
		);

		// Now do it for real.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				100,
				100, // just right
				10,
				100,
			)
		);

		let swap = Swaps::swaps(0).unwrap();

		// Balance left the sender...
		let sender_bal = Balances::free_balance(&1);
		assert_eq!(sender_bal, 10000 - 300);
		// ... and went into the swap account.
		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 300);

		// TokenId 0 left the sender...
		let sender_tokens = Fungible::balance_of((0, 1));
		assert_eq!(sender_tokens, 12);
		// .. and went into the swap account.
		let swap_tokens = Fungible::balance_of((0, swap.account));
		assert_eq!(swap_tokens, 30);

		// TokenId exists in sender's account.
		let sender_token_ones = Fungible::balance_of((1, 1));
		assert_eq!(sender_token_ones, 300);
	});
}

#[test]
fn remove_liquidity_fails_on_swap_with_no_liquidity() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Cannot remove liquidity from a swap with no liquidity.
		assert_noop!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				0,
				200, // shares to burn
				200, // min currency (exact)
				20, // min tokens (exact)
				100,
			),
			Error::<Test>::NoLiquidity
		);
	});
}

#[test]
fn it_removes_liquidity_from_swap() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				200, // Currency
				0,   // min swap shares
				20,	 // max tokens
				100,
			)
		);

		// First cause some no-ops
		// 0) NoSwapExists
		assert_noop!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				12, // this swap doesn't exist
				200,
				0,
				0,
				100,
			),
			Error::<Test>::NoSwapExists
		);
		// 1) BurnZeroShares
		assert_noop!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				0,
				0, // shares to burn
				0,
				0,
				100,
			),
			Error::<Test>::BurnZeroShares
		);
		// 2) NotEnoughCurrency
		assert_noop!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				0,
				200,
				2000, // min currency
				0,
				100,
			),
			Error::<Test>::NotEnoughCurrency
		);
		// 3) NotEnoughTokens
		assert_noop!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				0,
				200,
				0,
				2000, // min tokens
				100,
			),
			Error::<Test>::NotEnoughTokens
		);

		// Now successfully remove liquidity.
		assert_ok!(
			Swaps::remove_liquidity(
				Origin::signed(1),
				0,
				200, // shares to burn
				200, // min currency (exact)
				20, // min tokens (exact)
				100,
			)
		);

		// And make the requisite checks.
		let swap = Swaps::swaps(0).unwrap();

		// Sender has the same balance as the start.
		let sender_bal = Balances::free_balance(&1);
		assert_eq!(sender_bal, 10000);

		// Swap account has no balance (actual is now killed).
		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 0);

		// Sender has the same amount of TokenId 0.
		let sender_tokens = Fungible::balance_of((0, 1));
		assert_eq!(sender_tokens, 42);

		// Swap account has no tokens.
		let swap_tokens = Fungible::balance_of((0, swap.account));
		assert_eq!(swap_tokens, 0);

		// No shares exist.
		let shares_total_supply = Fungible::total_supply(1);
		assert_eq!(shares_total_supply, 0);
	});
}

#[test]
fn it_allows_swap_currency_to_tokens_input() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				420,
				0,
				42,
				100,
			)
		);

		assert_noop!(
		Swaps::currency_to_tokens_input(
			Origin::signed(2),
			0,
			300,
			20, // min tokens is set too high
			100,
			2
		),
		Error::<Test>::NotEnoughTokens
	);

		assert_ok!(
			Swaps::currency_to_tokens_input(
				Origin::signed(2),
				0,
				300,
				1,
				100,
				2
			)
		);

		let swap = Swaps::swaps(0).unwrap();

		let swapper_bal = Balances::free_balance(&2);
		assert_eq!(swapper_bal, 10000 - 300);

		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 720);

		let swapper_tokens = Fungible::balance_of((0, 2));
		assert_eq!(swapper_tokens, 17);

		let swap_tokens = Fungible::balance_of((0, &swap.account));
		assert_eq!(swap_tokens, 42 - 17);
	});
}

#[test]
fn it_allows_swap_currency_to_tokens_output() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				420,
				0,
				42,
				100,
			)
		);

		assert_noop!(
			Swaps::currency_to_tokens_output(
				Origin::signed(2),
				0,
				17,
				200, // max currency is too low for this token amount
				100,
				2
			),
			Error::<Test>::TooExpensiveCurrency
		);

		assert_ok!(
			Swaps::currency_to_tokens_output(
				Origin::signed(2),
				0,
				17,
				300, // just right
				100,
				2
			)
		);

		let swap = Swaps::swaps(0).unwrap();

		let swapper_bal = Balances::free_balance(&2);
		assert_eq!(swapper_bal, 10000 - 287);

		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 420 + 287);

		let swapper_tokens = Fungible::balance_of((0, 2));
		assert_eq!(swapper_tokens, 17);

		let swap_tokens = Fungible::balance_of((0, &swap.account));
		assert_eq!(swap_tokens, 42 - 17);
	});
}

#[test]
fn it_allows_tokens_to_currency_input() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				420,
				0,
				42,
				100,
			)
		);

		// Give some token to Account 2.
		assert_ok!(Fungible::mint(0, 2, 42));

		assert_noop!(
			Swaps::tokens_to_currency_input(
				Origin::signed(2),
				0,
				20, // tokens sold
				1000, // min currency too high
				100,
				2
			),
			Error::<Test>::NotEnoughCurrency,
		);
		
		assert_ok!(
			Swaps::tokens_to_currency_input(
				Origin::signed(2),
				0,
				20,
				1,
				100,
				2
			)
		);

		let swap = Swaps::swaps(0).unwrap();

		let swapper_tokens = Fungible::balance_of((0, 2));
		assert_eq!(swapper_tokens, 42 - 20);

		let swap_tokens = Fungible::balance_of((0, &swap.account));
		assert_eq!(swap_tokens, 42 + 20);

		let swapper_bal = Balances::free_balance(&2);
		assert_eq!(swapper_bal, 10000 + 135);

		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 420 - 135);
	});
}

#[test]
fn it_allows_tokens_to_currency_output() {
	new_test_ext().execute_with(|| {
		// Create TokenId 0.
		assert_ok!(Fungible::debug_create_token(Origin::signed(1), 42));

		// Create SwapId 0 for TokenId 0, creating TokenId 1 as shares.
		assert_ok!(Swaps::create_swap(Origin::signed(1), 0));

		// Adds liquidity to SwapId 0.
		assert_ok!(
			Swaps::add_liquidity(
				Origin::signed(1),
				0,
				420,
				0,
				42,
				100,
			)
		);

		assert_ok!(Fungible::mint(0, 2, 42));

		assert_noop!(
			Swaps::tokens_to_currency_output(
				Origin::signed(2),
				0,
				135, // currency bought
				1, // max_tokens too low
				100,
				2
			),
			Error::<Test>::TooExpensiveTokens,
		);
		
		assert_ok!(
			Swaps::tokens_to_currency_output(
				Origin::signed(2),
				0,
				135,
				1000,
				100,
				2
			)
		);

		let swap = Swaps::swaps(0).unwrap();

		let swapper_tokens = Fungible::balance_of((0, 2));
		assert_eq!(swapper_tokens, 42 - 20);

		let swap_tokens = Fungible::balance_of((0, &swap.account));
		assert_eq!(swap_tokens, 42 + 20);

		let swapper_bal = Balances::free_balance(&2);
		assert_eq!(swapper_bal, 10000 + 135);

		let swap_bal = Balances::free_balance(&swap.account);
		assert_eq!(swap_bal, 420 - 135);
	});
}
