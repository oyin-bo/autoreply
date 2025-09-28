# Storage rules for the BlueSky MCP stdio client

* Storage is based on filesystem
* Bulkiest data is stored in CAR/CBOR files in `./storage/did` sharded by first 2 characters of the DID
* Metadata and some metrics are in `./storage/didnt` sharded by ??

## DID

Storing account-centric data: mostly that's CAR,
but also AppView-derived metrics and lists.

```
./storage/
         /did/
             /00/
             /01/
             . . .
             /6f/
                /98761231/2024-03-01.car
                /98761231/2025-09-28.car
                /98761231/metrics.json
                /98761231/followed-by.json
                /...
```

