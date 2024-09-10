#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedEof,
    FromUtf8Error,
    ParseIntError,
    UnexpectedByte(u8),
    UnforeseenError,
}

#[derive(Debug, PartialEq)]
pub enum RespType {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Vec<RespType>),
}

impl RespType {
    pub fn from_bytes(bytes: &[u8]) -> Result<(&[u8], RespType), ParseError> {
        match bytes.get(0) {
            Some(b'+') => Self::read_string(&bytes[1..]),
            Some(b'-') => {
                // Here we simply call read_string and then convert the SimpleString to an Error
                // as they share the same format except the first byte
                let (bytes, resp) = Self::read_string(&bytes[1..])?;
                match resp {
                    RespType::SimpleString(resp) => Ok((bytes, RespType::Error(resp))),
                    _ => Err(ParseError::UnforeseenError),
                }
            }
            Some(b':') => {
                // Here we simply call read_string and then convert the SimpleString to an Integer
                // as they share the same format except the first byte
                let (bytes, resp) = Self::read_string(&bytes[1..])?;
                match resp {
                    RespType::SimpleString(resp) => {
                        let resp = resp.parse::<i64>().map_err(|_| ParseError::ParseIntError)?;
                        Ok((bytes, RespType::Integer(resp)))
                    }
                    _ => Err(ParseError::UnforeseenError),
                }
            }
            Some(b'$') => Self::read_bulk_string(&bytes[1..]),
            Some(b'*') => Self::read_array(&bytes[1..]),
            Some(byte) => Err(ParseError::UnexpectedByte(*byte))?,
            None => Err(ParseError::UnexpectedEof)?,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            RespType::SimpleString(string) => format!("+{}\r\n", string).into_bytes(),
            RespType::Error(error) => format!("-{}\r\n", error).into_bytes(),
            RespType::Integer(integer) => format!(":{}\r\n", integer).into_bytes(),
            RespType::BulkString(bulk) => {
                if let Some(bulk) = bulk {
                    let mut bytes = format!("${}\r\n", bulk.len())
                        .chars()
                        .map(|c| c as u8)
                        .collect::<Vec<u8>>();
                    bytes.extend(bulk);
                    bytes.extend(b"\r\n");
                    bytes
                } else {
                    b"$-1\r\n".to_vec()
                }
            }
            RespType::Array(array) => {
                let mut bytes = format!("*{}\r\n", array.len())
                    .chars()
                    .map(|c| c as u8)
                    .collect::<Vec<u8>>();
                for item in array {
                    bytes.extend(item.as_bytes());
                }
                bytes
            }
        }
    }

    fn read_string(bytes: &[u8]) -> Result<(&[u8], RespType), ParseError> {
        let (remaining, line) = Self::read_line(bytes)?;

        let string = String::from_utf8(line).map_err(|_| ParseError::FromUtf8Error)?;

        Ok((remaining, RespType::SimpleString(string)))
    }

    fn read_bulk_string(bytes: &[u8]) -> Result<(&[u8], RespType), ParseError> {
        let (remaining, line) = Self::read_line(bytes)?;

        let size = String::from_utf8(line)
            .map_err(|_| ParseError::FromUtf8Error)?
            .parse::<i64>()
            .map_err(|_| ParseError::ParseIntError)?;

        if size == -1 {
            return Ok((remaining, RespType::BulkString(None)));
        }

        let end_idx = size as usize + 2;
        if remaining.len() < end_idx {
            return Err(ParseError::UnexpectedEof);
        }

        let bulk_data = remaining[..size as usize].to_vec();
        let remaining = &remaining[end_idx..];

        Ok((remaining, RespType::BulkString(Some(bulk_data))))
    }

    fn read_array(bytes: &[u8]) -> Result<(&[u8], RespType), ParseError> {
        let (mut bytes, line) = Self::read_line(bytes)?;

        let size = String::from_utf8(line)
            .map_err(|_| ParseError::FromUtf8Error)?
            .parse::<i64>()
            .map_err(|_| ParseError::ParseIntError)?;

        let mut items: Vec<RespType> = Vec::new();
        for _ in 0..size {
            let (remaining, resp) = Self::from_bytes(&bytes)?;
            bytes = remaining;
            items.push(resp);
        }

        Ok((bytes, RespType::Array(items)))
    }

    fn read_line(bytes: &[u8]) -> Result<(&[u8], Vec<u8>), ParseError> {
        if let Some(position) = bytes.windows(2).position(|window| window == b"\r\n") {
            let line = &bytes[..position];
            let remaining = &bytes[position + 2..];
            Ok((remaining, line.to_vec()))
        } else {
            Err(ParseError::UnexpectedEof)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_bytes_with_empty_input() {
        let bytes = b"";
        let result = RespType::from_bytes(bytes);
        assert_eq!(result, Err(ParseError::UnexpectedEof));
    }

    #[test]
    fn test_from_bytes_one_byte() {
        let bytes = b"+";
        let result = RespType::from_bytes(bytes);
        assert_eq!(result, Err(ParseError::UnexpectedEof));
    }

    #[test]
    fn test_parse_simple_string() {
        let bytes = b"+OK\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(resp, RespType::SimpleString("OK".to_string()));
    }

    #[test]
    fn test_parse_error() {
        let bytes = b"-ERR unknown command 'foobar'\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Error("ERR unknown command 'foobar'".to_string())
        );
    }

    #[test]
    fn test_parse_integer() {
        let bytes = b":1000\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(resp, RespType::Integer(1000));
    }

    #[test]
    fn test_parse_bulk_string() {
        let bytes = b"$6\r\nfoobar\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::BulkString(Some("foobar".chars().map(|c| c as u8).collect::<Vec<u8>>()))
        );
    }

    #[test]
    fn test_parse_null_bulk_string() {
        let bytes = b"$-1\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(resp, RespType::BulkString(None));
    }

    #[test]
    fn test_parse_array() {
        let bytes = b"*2\r\n+foo\r\n+bar\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Array(vec![
                RespType::SimpleString("foo".to_string()),
                RespType::SimpleString("bar".to_string())
            ])
        );
    }

    #[test]
    fn test_parse_null_array() {
        let bytes = b"*-1\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(resp, RespType::Array(vec![]));
    }

    #[test]
    fn test_parse_nested_array() {
        let bytes = b"*2\r\n*2\r\n+foo\r\n+bar\r\n*2\r\n+foo\r\n+bar\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Array(vec![
                RespType::Array(vec![
                    RespType::SimpleString("foo".to_string()),
                    RespType::SimpleString("bar".to_string())
                ]),
                RespType::Array(vec![
                    RespType::SimpleString("foo".to_string()),
                    RespType::SimpleString("bar".to_string())
                ])
            ])
        );
    }

    #[test]
    fn test_parse_nested_null_array() {
        let bytes = b"*2\r\n*-1\r\n*-1\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Array(vec![RespType::Array(vec![]), RespType::Array(vec![])])
        );
    }

    #[test]
    fn test_parse_mixed_array() {
        let bytes = b"*3\r\n+foo\r\n:1000\r\n$6\r\nfoobar\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Array(vec![
                RespType::SimpleString("foo".to_string()),
                RespType::Integer(1000),
                RespType::BulkString(Some("foobar".chars().map(|c| c as u8).collect::<Vec<u8>>()))
            ])
        );
    }

    #[test]
    fn test_parse_mixed_null_array() {
        let bytes = b"*3\r\n+foo\r\n:1000\r\n$-1\r\n";
        let (_, resp) = RespType::from_bytes(bytes).unwrap();
        assert_eq!(
            resp,
            RespType::Array(vec![
                RespType::SimpleString("foo".to_string()),
                RespType::Integer(1000),
                RespType::BulkString(None)
            ])
        );
    }

    #[test]
    fn test_as_bytes_simple_string() {
        let resp = RespType::SimpleString("OK".to_string());
        let bytes = resp.as_bytes();
        assert_eq!(bytes, b"+OK\r\n");
    }

    #[test]
    fn test_as_bytes_error() {
        let resp = RespType::Error("ERR unknown command 'foobar'".to_string());
        let bytes = resp.as_bytes();
        assert_eq!(bytes, b"-ERR unknown command 'foobar'\r\n");
    }

    #[test]
    fn test_as_bytes_integer() {
        let resp = RespType::Integer(1000);
        let bytes = resp.as_bytes();
        assert_eq!(bytes, b":1000\r\n");
    }

    #[test]
    fn test_as_bytes_bulk_string() {
        let resp = RespType::BulkString(None);
        let bytes = resp.as_bytes();
        assert_eq!(bytes, b"$-1\r\n");
    }

    #[test]
    fn test_as_bytes_array() {
        let resp = RespType::Array(vec![
            RespType::SimpleString("foo".to_string()),
            RespType::SimpleString("bar".to_string()),
        ]);
        let bytes = resp.as_bytes();
        assert_eq!(bytes, b"*2\r\n+foo\r\n+bar\r\n");
    }
}
