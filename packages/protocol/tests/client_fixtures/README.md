# Independent Protobuf wire fixtures

The `.bin` files are generated from `.textproto` inputs by Google's `protoc`, independently of Rust/prost. Tests decode the payloads, assert important fields, and require byte-identical re-encoding. These are canonical Protobuf fixtures, not claimed to be TypeScript-generated fixtures.

Regenerate from the repository root:

```sh
protoc -I packages/protocol/proto --encode=gateway.ConnectRequest packages/protocol/proto/sdk/gateway.proto < packages/protocol/tests/client_fixtures/connect.textproto > packages/protocol/tests/client_fixtures/connect.bin
protoc -I packages/protocol/proto --encode=session.ClientEvent packages/protocol/proto/sdk/session.proto < packages/protocol/tests/client_fixtures/client_message.textproto > packages/protocol/tests/client_fixtures/client_message.bin
protoc -I packages/protocol/proto --encode=session.ServerEvent packages/protocol/proto/sdk/session.proto < packages/protocol/tests/client_fixtures/server_error.textproto > packages/protocol/tests/client_fixtures/server_error.bin
```

Coverage includes optional zero-valued layer limits, sender/room metadata, screen sources, binary messages, request IDs, and server error codes. The client uses these server definitions directly with default features disabled, so no schema copy needs synchronization.
