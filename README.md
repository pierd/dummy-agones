# dummy-agones

Dummy [Agones](https://github.com/googleforgames/agones) SDK server for testing Agones integration.

Note: `proto` comes directly from [the agones repository](https://github.com/googleforgames/agones/tree/main/proto).

## Usage

The server (started with `dummy-agones`) will serve HTTP traffic on port 9357 (Agones SDK default).

Apart from Agones SDK's gRPC it handles following endpoints:
- `GET /` - to retrieve current `GameServer` configuration as YAML
- `POST /` - with YAML data to update `GameServer` configuration

Retrieving configuration:
```sh
$ curl http://localhost:9357
---
objectMeta: ~
spec: ~
status: ~
```

Updating configuration (`--data-binary @file.yaml` can be handy):
```sh
$ curl -H 'Content-Type: application/yaml' --data-binary 'status: {state: "Allocated", address: "", ports: []}' http://localhost:9357
---
objectMeta: ~
spec: ~
status:
  state: Allocated
  address: ""
  ports: []
  players: ~
```

All changes are reflected in Agones SDK gRPC and all watchers are notified when there's a change.
