# Transaction Pool

Tonic transaction pool separated into 2 different pools.

## 1. Pending pool

Pending pool contains transactions that are executable with the current state of the mempool.

## 2. Queued pool

Queued pool contains transactions with:
1. out of order account nonces
2. not enough basefee with the current condition of the chain

