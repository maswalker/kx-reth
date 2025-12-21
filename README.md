# kasplex-reth

A high-performance Rust execution client for the Kasplex protocol, built on top of [Reth](https://github.com/paradigmxyz/reth) powerful [`NodeBuilder` API](https://reth.rs/introduction/why-reth#infinitely-customizable), designed to deliver the best possible developer and maintenance experience.

## Overview

kasplex-reth is a custom Reth-based client implementation for Kasplex L2 chains. Unlike a fork, this project leverages Reth's modular architecture and NodeBuilder API to provide Kasplex-specific features while maintaining compatibility with the Reth ecosystem.

## Key Features

- **Kasplex Chain Support**: Full support for all Kasplex networks (Mainnet, Internal L2, Testnet, Devnet)
- **Transaction Number System**: Implements Kasplex's transaction number mechanism for anchor transactions
- **Fixed Base Fee**: Uses a fixed base fee of 2000 GWei for all Kasplex chains
- **Treasury Address**: BaseFee is sent to a treasury address instead of being burnt
- **Custom RPC APIs**: Provides `kasplex` and `kasplexAuth` RPC namespaces
- **P2P Transaction Disabled**: P2P transaction submission is disabled for Kasplex networks

## Getting Started

### 1. Clone the Repository

```bash
git clone <repository-url>
cd kasplex-reth
```

### 2. Build

Build by `Cargo`:

```bash
cargo build --release
```

The main binary will be located at `target/release/kasplex-reth`.

### 3. Run Checks and Tests

To ensure everything is set up correctly, run the checks and tests:

```bash
cargo test
```

## Running the Node

To run the compiled node:

```bash
./target/release/kasplex-reth node \
    --chain kasplex-mainnet \
    --datadir /path/to/data \
    --http \
    --http.api eth,net,web3,kasplex,kasplexAuth \
    --http.addr 0.0.0.0 \
    --http.port 8545
```

To see available command-line options and subcommands, run:

```bash
./target/release/kasplex-reth --help
```

## Supported Kasplex Chains

| Chain Name | Chain ID | CLI Parameter |
|------------|----------|---------------|
| Kasplex Mainnet | 202555 | `--chain kasplex-mainnet` |
| Kasplex Internal L2 | 168001 | `--chain kasplex-internal-l2` |
| Kasplex Testnet | 168002 | `--chain kasplex-testnet` |
| Kasplex Devnet | 167012 | `--chain kasplex-devnet` |

## RPC APIs

### `kasplex` Namespace

- `kasplex_getSyncMode`: Get the node's sync mode

### `kasplexAuth` Namespace (Authenticated)

- `kasplexAuth_txPoolContent`: Get transaction pool content
- `kasplexAuth_txPoolContentWithMinTip`: Get transaction pool content with minimum tip
- `kasplexAuth_sendRawTransaction`: Send raw transaction
- `kasplexAuth_sendRawTransactions`: Send raw transactions in batch

**Note**: `eth_sendRawTransaction` is disabled for Kasplex networks. Use `kasplexAuth_sendRawTransaction` instead.

## Architecture

This project follows the same structure as `alethia-reth`:

- `bin/kasplex-reth/`: Main binary entry point
- `crates/chainspec/`: Kasplex chain specifications
- `crates/primitives/`: Kasplex-specific primitives (transaction Number, block Numbers)
- `crates/consensus/`: Kasplex consensus engine (fixed base fee, treasury)
- `crates/payload/`: Kasplex payload builder
- `crates/rpc/`: Kasplex RPC API implementations
- `crates/node/`: Kasplex node implementation
- `crates/cli/`: CLI interface

## Differences from Standard Ethereum

1. **Transaction Number**: Transactions have a `Number` field indicating the target block number
2. **Block Numbers Array**: Each block contains a `Numbers` array mapping transactions to their submission block numbers
3. **Fixed Base Fee**: Base fee is fixed at 2000 GWei (not calculated via EIP-1559)
4. **Treasury Address**: BaseFee is sent to a treasury address calculated from chain ID
5. **No P2P Transactions**: P2P transaction submission is disabled

## License

This project is licensed under the MIT OR Apache-2.0 License. See the [LICENSE](LICENSE) file for details.
