# Transaction Pool

Tonic transaction pool is separated into 3 different pools.

## 1. Pending pool

Pending pool contains transactions that are executable with the current state of the mempool.

Pending pool is of type PendingPool.

## 2. Queued pool

Queued pool contains transactions with:
1. out of order account nonces
2. not enough account balance for the transaction

Queued pool is of type ParkedPool.

## 3. Basefee pool

Basefee pool contains transactions that violates the current dynamic fee rates of the chain, but can be moved to pending in the future blocks if fees become lower.

Basefee pool is of type ParkedPool.
