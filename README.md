# RESP

This project implements a parser and serializer for the RESP (REdis Serialization Protocol), a simple protocol used by Redis for client-server communication. The library is written in Rust and provides functionality to parse RESP messages from raw byte streams as well as serialize responses back to RESP format.

## ToC
- Installation
- Supported types
- Examples

## Installation

Simply add the following under `[dependencies]` to your Cargo.toml.
```toml
resp = { git = "https://github.com/Palladium02/resp.git" }
```

## Supported types

The library supports the following RESP data types:

- SimpleString (+\<string>\r\n): A human-readable string.
- Error (-\<error message>\r\n): An error response from the server.
- Integer (:\<number>\r\n): A signed integer.
- BulkString ($\<length>\r\n<binary string>\r\n): A binary-safe string that may contain any data.
- Array (*\<size>\r\n\<elements>): A list of other RESP types.

## Examples

### Parsing a SimpleString

```rust
use resp::RespType;

let bytes = b"+OK\r\n";
let result = RespType::from_bytes(bytes);
assert_eq!(result, Ok((&[][..], RespType::SimpleString("OK".to_string()))));
```