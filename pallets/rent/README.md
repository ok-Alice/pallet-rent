# Rent Module

A simple and secure module for making non-fungible assets rentable.

[Pallet Rent Project Overview](https://michaelassaf.notion.site/Pallet-Rent-f3c3ecfce18d483eba9dea675721954d)

## Demo's
https://polkadot.js.org/apps/?rpc=wss%3A%2F%2Fpallet-rent-pgvftrncea-ew.a.run.app%3A443#/explorer

https://pallet-rent-character-loadout-pgvftrncea-ew.a.run.app/

## How to run

To test out the `pallet-rent` module, you can run the `xcodecraft/pallet-rent` docker image published to dockerhub using the following command.

`docker run -p 9944:9944 xcodecraft/pallet-rent --dev --unsafe-ws-external`

## Overview

The Rent module module provides functionality for non-fungible asset rental management, including:

- Asset Minting
- Asset Renting
- Asset Partial Ownership
- Asset Burning

To use it in your runtime, you need to implement the assets pallet_rent::Config.

The supported dispatchable functions are documented in the pallet_rent::Call enum.

### Terminology

- Non-fungible asset: An asset that is unique and can be identified by a unique identifier.
- Asset Minting: The process of creating a new non-fungible asset.
- Asset Renting: The process of renting a non-fungible asset.
- Asset Partial Ownership: The process of enabling partial ownership of a non-fungible asset ot a lessee.
- Asset Burning: The process of burning a non-fungible asset.
- Lessor: The account that owns a non-fungible asset and allows it to be rented.
- Lessee: The account that rents a non-fungible asset.

### Goals

The Pallet-Rent pallet in Substrate is designed to make the following possible:

- Allow an account to mint a non-fungible asset.
- Allow an account to rent out a non-fungible asset.
- Automatically process rent payments from the lessee to the lessor.
- Allow an account to define the rent payment method for a non-fungible asset.
- Allow an account to rent a non-fungible asset for a specified period of time (in blocks) with optional auto-renewal.
- Allow an account to burn a non-fungible asset.

## Interface

### Dispatchables

- `mint` - Mint a new non-fungible asset.
- `burn` - Destroy a non-fungible asset (only when there is no lessee - use `set_unrentable` and then `burn`).
- `set_rentable` - As a lessor, set a non-fungible asset available for rent.
- `set_unrentable` - As a lessor, set a non-fungible asset unavailable for rent.
- `rent` - As a lessee, rent a non-fungible asset.
- `set_recurring` - As a lessee, set a non-fungible asset to be rented on a recurring basis.
- `extend_rent` - As a lessee, extend the rental period of a non-fungible asset.

**Testing dispatchables**

- `equip_collectible` - Equip a non-fungible asset. (This is to demonstrate partial ownership)
- `unequip_collectible` - Unequip a non-fungible asset. (This is to demonstrate partial ownership)
