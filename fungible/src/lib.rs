#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::{
    Member, One, Zero, AtLeast32Bit, MaybeSerializeDeserialize, CheckedAdd,
    StaticLookup,
};

use frame_support::{
    decl_module, decl_storage, decl_event, decl_error, dispatch, ensure,
    Parameter,
};
use system::ensure_signed;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    type TokenBalance: Parameter + Member + AtLeast32Bit + Default + Copy
        + MaybeSerializeDeserialize;
    
    type TokenId: Parameter + Member + AtLeast32Bit + Default + Copy
        + MaybeSerializeDeserialize;
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        TokenBalance = <T as Trait>::TokenBalance,
        TokenId = <T as Trait>::TokenId,
    {
        NewToken(TokenId, AccountId, TokenBalance),
        /// <from, to, amount>
        Transfer(AccountId, AccountId, TokenBalance),
        /// <owner, spender, amount>
        Approval(AccountId, AccountId, TokenBalance),
    }
);

decl_error! {
    /// Errors for the fungible pallet.
    pub enum Error for Module<T: Trait> {
        /// Overflow during creation.
        CreationOverflow,
        /// Attempted to transfer zero tokens.
        TransferZeroAmount,
        /// Insufficient funds to make transfer.
        InsufficientFunds,
        /// Insufficient allowance to spend on behalf of an account.
        InsufficientAllowance,
    }
}

decl_storage!(
    trait Store for Module<T: Trait> as Fungible {
        TokenCount get(token_count): T::TokenId;

        /// ERC20 compatible.
        /// Maps (id, owner, spender) => amount.
        Allowance get(allowance): map hasher(opaque_blake2_256) (T::TokenId, T::AccountId, T::AccountId) => T::TokenBalance;
        Balances get(balance_of): map hasher(opaque_blake2_256) (T::TokenId, T::AccountId) => T::TokenBalance;
        TotalSupply get(total_supply): map hasher(opaque_blake2_256) T::TokenId => T::TokenBalance;
    }
);

decl_module!(
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {

        type Error = Error<T>;

        fn deposit_event() = default;

        pub fn debug_create_token(
            origin,
            #[compact] total_supply: T::TokenBalance,
        ) -> dispatch::DispatchResult 
        {
            let sender = ensure_signed(origin)?;

            let _id = Self::create_token(sender, total_supply);

            Ok(())
        }

        pub fn transfer(
            origin,
            id: T::TokenId,
            destination: <T::Lookup as StaticLookup>::Source,
            #[compact] amount: T::TokenBalance,
        ) -> dispatch::DispatchResult
        {
            let sender = ensure_signed(origin)?;
            let recipient = T::Lookup::lookup(destination)?;

            ensure!(!amount.is_zero(), Error::<T>::TransferZeroAmount);

            Self::do_transfer(id, sender.clone(), recipient.clone(), amount)
        }

        pub fn transfer_from(
            origin,
            id: T::TokenId,
            from: <T::Lookup as StaticLookup>::Source,
            to: <T::Lookup as StaticLookup>::Source,
            #[compact] amount: T::TokenBalance,
        ) -> dispatch::DispatchResult
        {
            let sender = ensure_signed(origin)?;
            let owner = T::Lookup::lookup(from)?;
            let recipient = T::Lookup::lookup(to)?;

            ensure!(!amount.is_zero(), Error::<T>::TransferZeroAmount);
            let allowed = Self::allowance((id, owner.clone(), sender.clone()));
            ensure!(allowed >= amount.clone(), Error::<T>::InsufficientAllowance);

            Self::do_transfer(id, owner.clone(), recipient.clone(), amount.clone())?;

            <Allowance<T>>::mutate((id, owner.clone(), sender.clone()), |allowed| {
                *allowed -= amount;
            });

            Ok(())
        }

        pub fn approve(
            origin,
            id: T::TokenId,
            spender: <T::Lookup as StaticLookup>::Source,
            #[compact] amount: T::TokenBalance,
        ) -> dispatch::DispatchResult
        {
            let sender = ensure_signed(origin)?;
            let a_spender = T::Lookup::lookup(spender)?;

            ensure!(!amount.is_zero(), Error::<T>::TransferZeroAmount);

            <Allowance<T>>::mutate((id, sender.clone(), a_spender.clone()), |allowed| {
                *allowed += amount.clone();
            });

            Self::deposit_event(RawEvent::Approval(sender.clone(), a_spender.clone(), amount));

            Ok(())
        }

        pub fn debug_mint(
            origin,
            id: T::TokenId,
            to: T::AccountId,
            amount: T::TokenBalance,
        ) -> dispatch::DispatchResult
        {
            ensure_signed(origin)?;
            Self::mint(id, to, amount)
        }

        pub fn debug_burn(origin, id: T::TokenId, from: T::AccountId, amount: T::TokenBalance) 
            -> dispatch::DispatchResult
        {
            ensure_signed(origin)?;
            Self::burn(id, from, amount)
        }
    }
);

impl<T: Trait> Module<T> {
    pub fn mint(id: T::TokenId, to: T::AccountId, amount: T::TokenBalance)
        -> dispatch::DispatchResult
    {
        <Balances<T>>::mutate((id, to), |bal| {
            *bal += amount.clone();
        });

        <TotalSupply<T>>::mutate(id, |sup| {
            *sup += amount;
        });

        Ok(())
    }

    pub fn burn(id: T::TokenId, from: T::AccountId, amount: T::TokenBalance)
        -> dispatch::DispatchResult
    {
        <Balances<T>>::mutate((id, from), |bal| {
            *bal -= amount.clone();
        });

        <TotalSupply<T>>::mutate(id, |sup| {
            *sup -= amount;
        });

        Ok(())
    }

    pub fn create_token(who: T::AccountId, total_supply: T::TokenBalance)
        -> T::TokenId
    {
        let id = Self::token_count();
        // TODO: Watch for overflow here. PUZZLE: Find a good solution that doesn't
        // need to make this function return a result, which may be an anti-pattern.
        let next_id = id.checked_add(&One::one()).unwrap();
        
        <Balances<T>>::insert((id, who.clone()), total_supply);
        <TotalSupply<T>>::insert(id, total_supply);
        <TokenCount<T>>::put(next_id);

        Self::deposit_event(RawEvent::NewToken(id, who, total_supply));
    
        id
    }

    pub fn do_transfer(id: T::TokenId, from: T::AccountId, to: T::AccountId, amount: T::TokenBalance)
        -> dispatch::DispatchResult
    {
        let from_balance = Self::balance_of((id, from.clone()));
        ensure!(
            from_balance >= amount.clone(),
            Error::<T>::InsufficientFunds,
        );

        <Balances<T>>::mutate((id, from.clone()), |balance| {
            *balance -= amount.clone();
        });
        <Balances<T>>::mutate((id, to.clone()), |balance| {
            *balance += amount.clone();
        });

        Self::deposit_event(RawEvent::Transfer(from.clone(), to.clone(), amount.clone()));

        Ok(())
    }
}
