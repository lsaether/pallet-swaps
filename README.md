# Swaps Pallet

Swaps is a [Substrate][substrate] pallet that implements the
Uniswap v1 functionality. It was mostly ported line-by-line from
the orginal Vyper smart contract code.

Currently the Swaps pallet is closely coupled to the home-baked
Fungible pallet (also in this repository) although that will
probably change over time when a standard for Substrate assets emerges.

## Tests

After cloning the repository, run `cargo test` to build the packages and run
the tests.

[substrate]: https://github.com/paritytech/substrate
