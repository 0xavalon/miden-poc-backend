# miden-poc-backend

Wind is developing a private 1-to-n payment solution using Miden's zero-knowledge (ZK) technology, enabling efficient, private payments to multiple recipients. It will integrate into Wind’s existing payment infrastructure

---

## Table of Contents

1. [Installation](#installation)
2. [Running the Project](#running-the-project)
3. [API Endpoints](#api-endpoints)
   - [Create Wallet](#create-wallet)
   - [Transfer Asset](#transfer-asset)
   - [Batch Transfer](#batch-transfer)
   - [Get Accounts](#get-accounts)

---

## Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- SQLite


### Clone the Repository
```bash
git clone https://github.com/0xavalon/miden-poc-backend.git
cd miden-poc-backend
```

# Running the Project
### Install Dependencies
Install Rust dependencies using cargo:
```
cargo build
```

### Start the Actix Web Server
```bash
cargo run
```
The server will run on http://127.0.0.1:8080

# API Endpoints
## Create Wallet
**Endpoint:** `GET /create-wallet`

Creates a new custodial wallet.

Request
```bash
curl --location 'http://127.0.0.1:8080/create-wallet' \
--header 'Content-Type: application/json' \
--request GET
```
Response
```json
{
    "account_type": "private",
    "wallet_id": "0x80f0e53763f05632"
}
```


## Transfer Asset
**Endpoint:** `POST /transfer`

Transfers assets from one wallet to another. Creates notes from sender account to target account.

Request
```bash
curl --location 'http://127.0.0.1:8080/transfer' \
--header 'Content-Type: application/json' \
--request POST \
--data '{
    "sender_wallet": "0x12345abcdef",
    "target_wallet": "0x67890fedcba",
    "amount": 100
}'
```

Response
```json
{
    "tx_id": "0x72e4e1b951d5b33ce3ab6d8368ce09505d1e37fe55073ff934e21f076e44f509",
    "sender_wallet": "0x12345abcdef",
    "target_wallet": "0x67890fedcba",
    "amount": 100
}
```


## Batch Transfer
**Endpoint:** `POST /batch-transfer`

Performs multiple transfers in a single API call.

Request
```bash
curl --location 'http://127.0.0.1:8080/batch-transfer' \
--header 'Content-Type: application/json' \
--request POST \
--data '{
    "transfers": [
        {
            "sender_wallet": "0x12345abcdef",
            "target_wallet": "0x67890fedcba",
            "amount": 100
        },
        {
            "sender_wallet": "0x54321fedcba",
            "target_wallet": "0x09876abcdef",
            "amount": 200
        }
    ]
}'

```

Response
```json
{
    "results": [
        {
            "sender_wallet": "0x12345abcdef",
            "target_wallet": "0x67890fedcba",
            "amount": 100,
            "tx_id": "0xabcdef1234567890",
            "error": null
        },
        {
            "sender_wallet": "0x54321fedcba",
            "target_wallet": "0x09876abcdef",
            "amount": 200,
            "tx_id": null,
            "error": "Invalid target wallet address"
        }
    ]
}
```


## Get Accounts
**Endpoint:** `GET /accounts`

Fetches all existing accounts with associated balance.

Request
```bash
curl --location 'http://127.0.0.1:8080/accounts' \
--header 'Content-Type: application/json' \
--request GET
```

Response
```json
{
    "accounts": [
        {
            "account_id": "0x82c2ad5c6bcbcfd5",
            "balance": 1000,
            "index": 0
        },
        {
            "account_id": "0x8c4b6a13872cb095",
            "balance": 500,
            "index": 1
        }
    ]
}
```