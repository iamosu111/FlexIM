- [ ] ToDoList 
    - [X] MB-Tree
    - [ ] CMAB
    - [ ] bloom filter+ budget allocation
    - [ ] query 
    - [ ] query verify
    - [ ] inter-index
    - [ ] index cost evaluation
    - [ ] Root Merkle Tree

## Necessary Knowledge

```
---compile code
cargo test
cargo build --release

```




## SimChain

#### Input format

```
block_id [address] {in/out, amount, timestamp}
```

For example

```
1 [muhtvdmsnbQEPFuEmxcChX58fGvXaaUoVt] {in, 50, 1571443461}
1 [mwhtvdmsnbQEPFuEmxcChX58fGvXaaUoVt] {in, 50, 1571443461}
1 [mvbnrCX3bg1cDRUu8pkecrvP6vQkSLDSou] {out, 10, 1571443461}
```

### Build Chain

Run `simchain-build` to build the chain. The default value of learned index error bounds is set to be 5.

```
./simchain-build -i data/input.txt -d data/db
```

Run `simchain-build -h` for more info.

### Deploy Chain

Run `simchain-server` after `simchain-build` is taken.

```
./simchain-server -d data/db 
```

Simchain's port is set to 8000 on default.

### Service API

Use RESTFul API to inspect the blockchain.

```
GET /get/param
GET /get/blk_header/{id}
GET /get/blk_data/{id}
GET /get/tx/{id}
```

For example, if a server is running on port 8000 locally, then the get_param request will be as followed in Linux

```
curl -X GET http:127.0.0.1:8000/get/param
```


