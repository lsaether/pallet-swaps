#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Encode, Decode};
use sp_runtime::{ModuleId};
use sp_runtime::traits::{
    Member, One, Zero, AtLeast32Bit, MaybeSerializeDeserialize, CheckedAdd,
    AccountIdConversion, SaturatedConversion,
};

use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch,
	ensure, Parameter, traits::{Currency, ExistenceRequirement},
};
use system::ensure_signed;

use pallet_fungible::{self as fungible};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(Clone, Eq, PartialEq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Swap<AccountId, TokenId> {
	// The token being swapped.
	token_id: TokenId,
	// The "swap token" id.
	swap_token: TokenId,
	// This swap account.
	account: AccountId,
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// The swap's module id, used for deriving sovereign account IDs.
const MODULE_ID: ModuleId = ModuleId(*b"mtg/swap");

/// The pallet's configuration trait.
pub trait Trait: system::Trait + fungible::Trait {

	/// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    
    type SwapId: Parameter + Member + AtLeast32Bit + Default + Copy
		+ MaybeSerializeDeserialize;

	type Currency: Currency<Self::AccountId>;
}

// Storage items for the Swap pallet.
decl_storage! {
	trait Store for Module<T: Trait> as SwapStorage {
		TokenToSwap get(token_to_swap): map hasher(opaque_blake2_256) T::TokenId => T::SwapId;
		Swaps get(swaps): map hasher(opaque_blake2_256) T::SwapId => Option<Swap<T::AccountId, T::TokenId>>;
		SwapCount get(swap_count): T::SwapId;
	}
}

// Events for the Swap pallet.
decl_event!(
	pub enum Event<T> 
	where
		AccountId = <T as system::Trait>::AccountId,
		BalanceOf = BalanceOf<T>,
		Id = <T as Trait>::SwapId,
		TokenBalance = <T as fungible::Trait>::TokenBalance
	{
		/// Logs (SwapId, SwapAccount)
		SwapCreated(Id, AccountId),
		/// Logs (SwapId, x, x, x)
		LiquidityAdded(Id, AccountId, BalanceOf, TokenBalance),
		/// Logs (SwapId, x, x, x)
		LiquidityRemoved(Id, AccountId, BalanceOf, TokenBalance),
		/// Logs (SwapId, buyer, currency_bought, tokens_sold, recipient)
		CurrencyPurchase(),
		/// Logs (SwapId, buyer, currency_sold, tokens_bought, recipient)
		TokenPurchase(),
	}
);

// Errors for the Swap pallet.
decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Deadline hit.
		Deadline,
		/// Zero tokens supplied.
		ZeroTokens,
		/// Zero reserve supplied.
		ZeroAmount,
		/// No Swap exists at this Id.
		NoSwapExists,
		/// A Swap already exists for a particular TokenId.
		SwapAlreadyExists,
		/// Requested zero liquidity.
		RequestedZeroLiquidity,
		/// Would add too many tokens to liquidity.
		TooManyTokens,
		/// Not enough liquidity created.
		TooLowLiquidity,
		/// No currency is being swapped.
		NoCurrencySwapped,
		/// No tokens are being swapped.
		NoTokensSwapped,
		/// Trying to burn zero shares.
		BurnZeroShares,
		/// No liquidity in the swap.
		NoLiquidity,
		/// Not enough currency will be returned.
		NotEnoughCurrency,
		/// Not enough tokens will be returned.
		NotEnoughTokens,
		/// Swap would cost too much in currency.
		TooExpensiveCurrency,
		/// Swap would cost too much in tokens.
		TooExpensiveTokens,
	}
}

// The pallet's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		type Error = Error<T>;

		fn deposit_event() = default;
		
		pub fn create_swap(origin,
			token_id: T::TokenId,
		) -> dispatch::DispatchResult
		{
			let sender = ensure_signed(origin)?;
			ensure!(!TokenToSwap::<T>::contains_key(token_id), Error::<T>::SwapAlreadyExists);

			let swap_id = Self::swap_count();
			let next_id = swap_id.checked_add(&One::one())
				.ok_or("Overflow")?;

			let swap_token_id = fungible::Module::<T>::create_token(sender, Zero::zero());

			let account: T::AccountId = MODULE_ID.into_sub_account(swap_token_id);

			let new_swap = Swap {
				token_id: token_id,
				swap_token: swap_token_id,
				account: account.clone(),
			};

			<TokenToSwap<T>>::insert(token_id, swap_id);
			<Swaps<T>>::insert(swap_id, new_swap);
			<SwapCount<T>>::put(next_id);

			Self::deposit_event(RawEvent::SwapCreated(swap_id, account));

			Ok(())
		}
        
        pub fn add_liquidity(origin,
			swap_id: T::SwapId,				// ID of swap to access.
			currency_amount: BalanceOf<T>,  // Amount of base currency to lock.
            min_liquidity: T::TokenBalance,	// Min amount of swap shares to create.
			max_tokens: T::TokenBalance,	// Max amount of tokens to input.
            deadline: T::BlockNumber,		// When to invalidate the transaction.
        ) -> dispatch::DispatchResult
        {
			// Deadline is to prevent front-running (more of a problem on Ethereum).
			let now = system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let who = ensure_signed(origin.clone())?;

			ensure!(max_tokens > Zero::zero(), Error::<T>::ZeroTokens);
			ensure!(currency_amount > Zero::zero(), Error::<T>::ZeroAmount);

			if let Some(swap) = Self::swaps(swap_id) {
				let total_liquidity = fungible::Module::<T>::total_supply(swap.swap_token.clone());

				if total_liquidity > Zero::zero() {
					ensure!(min_liquidity > Zero::zero(), Error::<T>::RequestedZeroLiquidity);
					let swap_balance = Self::convert(Self::get_swap_balance(&swap));
					let token_reserve = Self::get_token_reserve(&swap);
					let token_amount = Self::convert(currency_amount) * token_reserve / swap_balance;
					let liquidity_minted = Self::convert(currency_amount) * total_liquidity / swap_balance;

					ensure!(max_tokens >= token_amount, Error::<T>::TooManyTokens);
					ensure!(liquidity_minted >= min_liquidity, Error::<T>::TooLowLiquidity);

					T::Currency::transfer(&who, &swap.account, currency_amount, ExistenceRequirement::KeepAlive)?;
					fungible::Module::<T>::mint(swap.swap_token.clone(), who.clone(), liquidity_minted)?;
					fungible::Module::<T>::do_transfer(swap.token_id, who.clone(), swap.account, token_amount)?;
					Self::deposit_event(RawEvent::LiquidityAdded(swap_id, who.clone(), currency_amount.clone(), token_amount));
				} else {
					// Fresh swap with no liquidity ~
					let token_amount = max_tokens;
					let this = swap.account.clone();
					T::Currency::transfer(&who, &swap.account, currency_amount, ExistenceRequirement::KeepAlive)?;
					let initial_liquidity: u64 = T::Currency::free_balance(&this).saturated_into::<u64>();
					fungible::Module::<T>::mint(swap.swap_token.clone(), who.clone(), initial_liquidity.saturated_into())?;
					fungible::Module::<T>::do_transfer(swap.token_id, who.clone(), this.clone(), token_amount)?;
					Self::deposit_event(RawEvent::LiquidityAdded(swap_id, who, currency_amount, token_amount));
				}

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}
		
		pub fn remove_liquidity(origin,
			swap_id: T::SwapId,
			shares_to_burn: T::TokenBalance, 
			min_currency: BalanceOf<T>,		// Minimum currency to withdraw.
			min_tokens: T::TokenBalance,	// Minimum tokens to withdraw.
			deadline: T::BlockNumber,
		) -> dispatch::DispatchResult
		{
			let now = system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let who = ensure_signed(origin.clone())?;

			ensure!(shares_to_burn > Zero::zero(), Error::<T>::BurnZeroShares);

			if let Some(swap) = Self::swaps(swap_id) {
				let total_liquidity = fungible::Module::<T>::total_supply(swap.swap_token.clone());

				ensure!(total_liquidity > Zero::zero(), Error::<T>::NoLiquidity);

				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_amount = shares_to_burn.clone() * Self::convert(swap_balance) / total_liquidity.clone();
				let token_amount = shares_to_burn.clone() * token_reserve / total_liquidity.clone();

				ensure!(Self::unconvert(currency_amount) >= min_currency, Error::<T>::NotEnoughCurrency);
				ensure!(token_amount >= min_tokens, Error::<T>::NotEnoughTokens);

				fungible::Module::<T>::burn(swap.swap_token.clone(), who.clone(), shares_to_burn)?;

				T::Currency::transfer(&swap.account, &who, Self::unconvert(currency_amount), ExistenceRequirement::AllowDeath)?;
				// Need to ensure this happens.
				fungible::Module::<T>::do_transfer(swap.token_id, swap.account.clone(), who.clone(), token_amount.clone())?;
				
				Self::deposit_event(RawEvent::LiquidityRemoved(swap_id, who, Self::unconvert(currency_amount), token_amount));

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts currency to tokens.
		///
		/// User specifies the exact amount of currency to spend and the minimum
		/// tokens to be returned.
		pub fn currency_to_tokens_input(origin,
			swap_id: T::SwapId,
			currency: BalanceOf<T>,
			min_tokens: T::TokenBalance,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) -> dispatch::DispatchResult
		{
			let now = system::Module::<T>::block_number();
			ensure!(deadline > now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(currency > Zero::zero(), Error::<T>::NoCurrencySwapped);
			ensure!(min_tokens > Zero::zero(), Error::<T>::NoTokensSwapped);

			if let Some(swap) = Self::swaps(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let tokens_bought = Self::get_input_price(Self::convert(currency), Self::convert(swap_balance), token_reserve);
				
				ensure!(tokens_bought >= min_tokens, Error::<T>::NotEnoughTokens);
				
				T::Currency::transfer(&buyer, &swap.account, currency, ExistenceRequirement::KeepAlive)?;
				fungible::Module::<T>::do_transfer(swap.token_id, swap.account.clone(), recipient, tokens_bought)?;

				Self::deposit_event(RawEvent::TokenPurchase());

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts currency to tokens.
		///
		/// User specifies the maximum currency to spend and the exact amount of
		/// tokens to be returned.
		pub fn currency_to_tokens_output(origin,
			swap_id: T::SwapId,
			tokens_bought: T::TokenBalance,
			max_currency: BalanceOf<T>,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) -> dispatch::DispatchResult
		{
			let now = system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(tokens_bought > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(max_currency > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_sold = Self::get_output_price(tokens_bought, Self::convert(swap_balance), token_reserve);

				ensure!(Self::unconvert(currency_sold) <= max_currency, Error::<T>::TooExpensiveCurrency);

				T::Currency::transfer(&buyer, &swap.account, Self::unconvert(currency_sold), ExistenceRequirement::KeepAlive)?;
				fungible::Module::<T>::do_transfer(swap.token_id, swap.account.clone(), recipient, tokens_bought)?;
				
				Self::deposit_event(RawEvent::TokenPurchase());

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts tokens to currency.
		///
		/// The user specifies exact amount of tokens sold and minimum amount of
		/// currency that is returned.
		pub fn tokens_to_currency_input(origin,
			swap_id: T::SwapId,
			tokens_sold: T::TokenBalance,
			min_currency: BalanceOf<T>,
			deadline: T:: BlockNumber,
			recipient: T::AccountId,
		) -> dispatch::DispatchResult
		{
			let now = system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(tokens_sold > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(min_currency > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let currency_bought = Self::get_input_price(tokens_sold, token_reserve, Self::convert(swap_balance));

				ensure!(currency_bought >= Self::convert(min_currency), Error::<T>::NotEnoughCurrency);

				T::Currency::transfer(&swap.account, &recipient, Self::unconvert(currency_bought), ExistenceRequirement::AllowDeath)?;
				fungible::Module::<T>::do_transfer(swap.token_id, buyer, swap.account, tokens_sold)?;
				
				Self::deposit_event(RawEvent::CurrencyPurchase());

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}

		/// Converts tokens to currency.
		///
		/// The user specifies the maximum tokens to swap and the exact
		/// currency to be returned.
		pub fn tokens_to_currency_output(origin,
			swap_id:  T::SwapId,
			currency_bought: BalanceOf<T>,
			max_tokens: T::TokenBalance,
			deadline: T::BlockNumber,
			recipient: T::AccountId,
		) -> dispatch::DispatchResult
		{
			let now = system::Module::<T>::block_number();
			ensure!(deadline >= now, Error::<T>::Deadline);

			let buyer = ensure_signed(origin)?;

			ensure!(max_tokens > Zero::zero(), Error::<T>::NoTokensSwapped);
			ensure!(currency_bought > Zero::zero(), Error::<T>::NoCurrencySwapped);

			if let Some(swap) = Self::swaps(swap_id) {
				let token_reserve = Self::get_token_reserve(&swap);
				let swap_balance = Self::get_swap_balance(&swap);
				let tokens_sold = Self::get_output_price(Self::convert(currency_bought), token_reserve, Self::convert(swap_balance));

				ensure!(max_tokens >= tokens_sold, Error::<T>::TooExpensiveTokens);

				T::Currency::transfer(&swap.account, &buyer, currency_bought, ExistenceRequirement::AllowDeath)?;
				fungible::Module::<T>::do_transfer(swap.token_id, recipient, swap.account, tokens_sold)?;
				
				Self::deposit_event(RawEvent::CurrencyPurchase());

				Ok(())
			} else {
				Err(Error::<T>::NoSwapExists)?
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn get_currency_to_token_input_price(swap: &Swap<T::AccountId, T::TokenId>, currency_sold: BalanceOf<T>)
		-> T::TokenBalance
	{
		if currency_sold == Zero::zero() { return Zero::zero(); }

		let token_reserve = Self::get_token_reserve(swap);
		let swap_balance = Self::get_swap_balance(swap);
		Self::get_input_price(Self::convert(currency_sold), Self::convert(swap_balance), token_reserve)
	}

	// pub fn get_currency_to_token_output_price(swap: &Swap<T::AccountId, T::TokenId>, tokens_bought: T::TokenBalance)
	// 	-> T::TokenBalance
	// {

	// }

	// pub fn get_token_to_currency_input_price(swap: &Swap<T::AccountId, T::TokenId>, tokens_sold: T::TokenBalance)
	// 	-> T::TokenBalance
	// {

	// }

	// pub fn get_token_to_currency_output_price(swap: &Swap<T::AccountId, T::TokenId>, currency_bought: BalanceOf<T>)
	// 	-> T::TokenBalance
	// {

	// }

	fn get_output_price(
		output_amount: T::TokenBalance,
		input_reserve: T::TokenBalance,
		output_reserve: T::TokenBalance,
	) -> T::TokenBalance
	{
		let numerator = input_reserve * output_amount * 1000.into();
		let denominator = (output_reserve - output_amount) * 997.into();
		numerator / denominator + 1.into()
	}

	fn get_input_price(
		input_amount: T::TokenBalance,
		input_reserve: T::TokenBalance,
		output_reserve: T::TokenBalance,
	) -> T::TokenBalance
	{
		let input_amount_with_fee = input_amount * 997.into();
		let numerator = input_amount_with_fee * output_reserve;
		let denominator = (input_reserve * 1000.into()) + input_amount_with_fee;
		numerator / denominator
	}

	fn convert(balance_of: BalanceOf<T>) -> T::TokenBalance {
		let m = balance_of.saturated_into::<u64>();
		m.saturated_into()
	}

	fn unconvert(token_balance: T::TokenBalance) -> BalanceOf<T> {
		let m = token_balance.saturated_into::<u64>();
		m.saturated_into()
	}

	fn get_token_reserve(swap: &Swap<T::AccountId, T::TokenId>) -> T::TokenBalance {
		fungible::Module::<T>::balance_of((swap.token_id.clone(), &swap.account))
	}

	fn get_swap_balance(swap: &Swap<T::AccountId, T::TokenId>) -> BalanceOf<T> {
		T::Currency::free_balance(&swap.account)
	}
}
