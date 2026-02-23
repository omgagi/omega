---
name = "doli-miner"
description = "DOLI blockchain producer node management — wallet, mining, rewards, node health, service setup."
trigger = "doli|miner|producer|bond|wallet|rewards|epoch|node|block|mining"
---

# DOLI Miner

Day-to-day operations for a DOLI blockchain producer (miner).

## What it covers

- **Wallet** — create, check balance, send DOLI, transaction history
- **Producer** — register, add bonds, check status, claim withdrawals
- **Rewards** — list claimable epochs, claim rewards, check epoch progress
- **Node health** — chain status, peer count, sync state, process check
- **Service** — create and manage systemd (Linux) or launchd (macOS) service
- **Troubleshooting** — common issues, clock drift, RocksDB locks, resync

## Install

```bash
# Prerequisites — Rust (minimum 1.85)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# System deps (macOS)
xcode-select --install
brew install gmp cmake protobuf

# System deps (Ubuntu/Debian)
sudo apt install -y build-essential cmake pkg-config libssl-dev \
  libgmp-dev protobuf-compiler clang libclang-dev

# Clone and build
git clone https://github.com/e-weil/doli.git
cd doli
cargo build --release

# Verify
./target/release/doli-node --version
./target/release/doli --version
```

Binaries: `doli-node` (full node) and `doli` (wallet CLI), both in `target/release/`.

Data directories: `~/.doli/mainnet/`, `~/.doli/testnet/`, `~/.doli/devnet/`.

### First run

```bash
./target/release/doli new              # 1. create wallet
./target/release/doli-node run --yes   # 2. start node (syncs mainnet)
./target/release/doli chain            # 3. check sync progress
./target/release/doli balance          # 4. check balance once synced
```

## Address format

Bech32m: `doli1...` (mainnet), `tdoli1...` (testnet), `ddoli1...` (devnet).

## CLI flag order (critical)

Global options go BEFORE the subcommand:

```bash
# CORRECT
doli-node --data-dir /path run --producer
# WRONG
doli-node run --data-dir /path
```

## Wallet

```bash
doli new --name producer-wallet   # create wallet
doli info                         # show address + pubkey
doli balance                      # confirmed / unconfirmed / immature
doli send doli1recipient... 100   # send coins
doli history                      # last 10 transactions
doli export /path/to/backup.json  # backup wallet
doli import /path/to/backup.json  # restore wallet
```

Custom RPC: `doli -r http://127.0.0.1:28545 balance` (devnet).

## Producer

```bash
doli producer register -b 1          # register with 1 bond (10 DOLI mainnet, 1 DOLI devnet)
doli producer status                 # own status: active/unbonding/exited, bond count, pending withdrawals
doli producer add-bond --count 3     # add 3 more bonds (more bonds = more block production slots)
doli producer request-withdrawal --count 2   # start 7-day unbonding (mainnet) / 10 min (devnet)
doli producer claim-withdrawal       # claim after delay period
doli producer exit                   # check early exit penalty before committing
```

Bond economics:
- Mainnet: 10 DOLI per bond, 7-day unbond delay
- Devnet: 1 DOLI per bond, 10 min unbond delay
- Early exit penalty: <1yr=75%, 1-2yr=50%, 2-3yr=25%, 3yr+=0%

## Rewards

```bash
doli rewards list                              # show unclaimed epochs with estimated rewards
doli rewards claim 42                          # claim specific epoch
doli rewards claim-all                         # claim all pending epochs
doli rewards history                           # past claims
doli rewards info                              # current epoch progress, blocks remaining
```

Reward rules:
- 100% to producer (no split)
- Block reward halves every era (~4 years)
- Epoch = 360 blocks (mainnet) / 60 blocks (devnet)
- Coinbase maturity: 100 blocks (mainnet) / 10 blocks (devnet)

## Node

```bash
# Run producer node (mainnet)
./target/release/doli-node --data-dir ~/.doli/mainnet/data run \
  --producer --producer-key ~/.doli/mainnet/keys/producer.json \
  --yes --force-start

# Run non-producer (sync only)
./target/release/doli-node run --yes

# Chain status
doli chain

# RPC health check
curl -s -X POST http://127.0.0.1:8545 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"getChainInfo","params":{},"id":1}' | jq .
```

Network ports:

| Network | P2P | RPC | Metrics |
|---------|-----|-----|---------|
| Mainnet | 30303 | 8545 | 9090 |
| Testnet | 40303 | 18545 | 19090 |
| Devnet | 50303 | 28545 | 29090 |

## Service setup

Auto-detect OS and create the appropriate service:

**Linux (systemd):**
```bash
sudo systemctl start doli-mainnet     # start
sudo systemctl stop doli-mainnet      # stop
sudo systemctl status doli-mainnet    # status
journalctl -u doli-mainnet -f         # logs
```

**macOS (launchd):**
```bash
launchctl load ~/Library/LaunchAgents/network.doli.mainnet.plist    # start
launchctl stop network.doli.mainnet                                 # stop
launchctl list | grep doli                                          # status
tail -f ~/.doli/mainnet/node.log                                    # logs
```

## Troubleshooting

**Node not producing blocks:**
1. `doli producer status` — verify active
2. Check process has `--producer` flag: `pgrep -la doli-node`
3. Check peer count > 0: `doli chain`
4. Check clock sync: `date -u` vs `ntpdate -q pool.ntp.org`

**No peers:** Open P2P port (30303), verify bootstrap node.

**RocksDB LOCK error:** Another `doli-node` process is running. Kill it first — never delete the LOCK file.

**Clock drift:** Max allowed is 1 second. Fix: `sudo sntp -sS pool.ntp.org` (macOS) or `sudo timedatectl set-ntp true` (Linux).

**Stuck sync:** Restart node. If stuck at height 0, check bootstrap and firewall.

**Fork/corruption:** Stop node, delete state files (keep keys!), restart to resync from peers:
```bash
rm -f chain_state.bin producers.bin utxo.bin
rm -rf blocks/ signed_slots.db/
```
