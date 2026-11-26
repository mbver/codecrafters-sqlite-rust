use std::io::{self, Read};

use crate::sql::ast::Literal;

#[allow(dead_code)]
pub struct Cell {
    pub payload_size: u64,
    pub row_id: u64,
    pub record: Record,
}
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum Value {
    Null,
    Integer(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Integer(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Text(value) => write!(f, "{value}"),
            Value::Blob(value) => write!(f, "{value:?}"),
        }
    }
}
impl TryFrom<Literal> for Value {
    type Error = String;

    fn try_from(literal: Literal) -> Result<Self, Self::Error> {
        match literal {
            Literal::String(value) => Ok(Value::Text(value)),

            Literal::Number(value) => {
                if let Ok(integer) = value.parse::<i64>() {
                    Ok(Value::Integer(integer))
                } else if let Ok(float) = value.parse::<f64>() {
                    Ok(Value::Float(float))
                } else {
                    Err(format!("Invalid numeric literal: {value}"))
                }
            }

            Literal::Null => Ok(Value::Null),
        }
    }
}
use std::cmp::Ordering;

impl Value {
    pub fn compare(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Null, Value::Null) => Some(Ordering::Equal),

            (Value::Null, _) => Some(Ordering::Less),
            (_, Value::Null) => Some(Ordering::Greater),

            (Value::Integer(a), Value::Integer(b)) => Some(a.cmp(b)),

            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),

            (Value::Integer(a), Value::Float(b)) => (*a as f64).partial_cmp(b),

            (Value::Float(a), Value::Integer(b)) => a.partial_cmp(&(*b as f64)),

            (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),

            (Value::Blob(a), Value::Blob(b)) => Some(a.cmp(b)),

            (Value::Integer(_) | Value::Float(_), Value::Text(_) | Value::Blob(_)) => {
                Some(Ordering::Less)
            }

            (Value::Text(_) | Value::Blob(_), Value::Integer(_) | Value::Float(_)) => {
                Some(Ordering::Greater)
            }

            (Value::Text(_), Value::Blob(_)) => Some(Ordering::Less),

            (Value::Blob(_), Value::Text(_)) => Some(Ordering::Greater),
        }
    }
}
impl Value {
    pub fn into_text(self) -> io::Result<String> {
        match self {
            Value::Text(value) => Ok(value),
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }

    pub fn into_integer(self) -> io::Result<i64> {
        match self {
            Value::Integer(value) => Ok(value),
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }

    pub fn into_optional_text(self) -> io::Result<Option<String>> {
        match self {
            Value::Text(value) => Ok(Some(value)),
            Value::Null => Ok(None),
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}
pub struct Record {
    pub values: Vec<Value>,
}

impl TryFrom<Record> for SchemaRecord {
    type Error = io::Error;

    fn try_from(record: Record) -> io::Result<Self> {
        let mut values = record.values.into_iter();

        let record_type = values
            .next()
            .ok_or(io::ErrorKind::InvalidData)?
            .into_text()?;
        let name = values
            .next()
            .ok_or(io::ErrorKind::InvalidData)?
            .into_text()?;
        let tbl_name = values
            .next()
            .ok_or(io::ErrorKind::InvalidData)?
            .into_text()?;
        let rootpage = values
            .next()
            .ok_or(io::ErrorKind::InvalidData)?
            .into_integer()?;
        let sql = values
            .next()
            .ok_or(io::ErrorKind::InvalidData)?
            .into_optional_text()?;

        Ok(Self {
            record_type,
            name,
            tbl_name,
            rootpage,
            sql,
        })
    }
}

#[allow(dead_code)]
pub struct SchemaRecord {
    pub record_type: String,
    pub name: String,
    pub tbl_name: String,
    pub rootpage: i64,
    pub sql: Option<String>,
}

fn read_varint(data: &[u8], offset: &mut usize) -> io::Result<u64> {
    let mut value = 0u64;

    for _ in 0..8 {
        let byte = *data
            .get(*offset)
            .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;

        *offset += 1;
        value = (value << 7) | (byte & 0x7f) as u64;

        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }

    let byte = *data
        .get(*offset)
        .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;

    *offset += 1;

    Ok((value << 8) | byte as u64)
}

pub fn read_varint_from_reader(reader: &mut impl Read) -> io::Result<u64> {
    let mut value = 0u64;

    for _ in 0..8 {
        let mut buffer = [0u8; 1];
        reader.read_exact(&mut buffer)?;

        let byte = buffer[0];
        value = (value << 7) | (byte & 0x7f) as u64;

        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }

    let mut buffer = [0u8; 1];
    reader.read_exact(&mut buffer)?;

    Ok((value << 8) | buffer[0] as u64)
}

pub fn read_cell(reader: &mut impl Read) -> io::Result<Cell> {
    let payload_size = read_varint_from_reader(reader)?;
    let row_id = read_varint_from_reader(reader)?;

    let mut payload = vec![0u8; payload_size as usize];
    reader.read_exact(&mut payload)?;

    let record = read_record(&payload)?;

    Ok(Cell {
        payload_size,
        row_id,
        record,
    })
}
fn read_record(data: &[u8]) -> io::Result<Record> {
    let mut offset = 0;

    let header_size = read_varint(data, &mut offset)? as usize;
    let header_end = header_size;

    let mut serial_types = Vec::new();

    while offset < header_end {
        serial_types.push(read_varint(data, &mut offset)?);
    }

    let mut values = Vec::new();

    for serial_type in serial_types {
        values.push(read_value(data, &mut offset, serial_type)?);
    }

    Ok(Record { values })
}

fn read_value(data: &[u8], offset: &mut usize, serial_type: u64) -> io::Result<Value> {
    let length = serial_type_length(serial_type)?;

    let end = *offset + length;
    let bytes = data
        .get(*offset..end)
        .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;

    *offset = end;

    match serial_type {
        0 => Ok(Value::Null),

        1..=6 => {
            let mut value = 0u64;

            for &byte in bytes {
                value = (value << 8) | byte as u64;
            }

            // Sign extension for integers shorter than 8 bytes.
            // most significant bit is 1
            if length < 8 && bytes[0] & 0x80 != 0 {
                value |= u64::MAX << (length * 8); // create mask for higher bits
            }

            Ok(Value::Integer(value as i64))
        }

        7 => {
            let bits = u64::from_be_bytes(bytes.try_into().unwrap());
            Ok(Value::Float(f64::from_bits(bits)))
        }

        8 => Ok(Value::Integer(0)),
        9 => Ok(Value::Integer(1)),

        10 | 11 => Err(io::Error::from(io::ErrorKind::InvalidData)),

        n if n >= 12 && n % 2 == 0 => Ok(Value::Blob(bytes.to_vec())), // blob: (n-12)/2

        n if n >= 13 && n % 2 == 1 => {
            let text = String::from_utf8(bytes.to_vec())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?; // text: (n-13)/2

            Ok(Value::Text(text))
        }

        _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
    }
}
/*
*  serial_type describes the stored value's type and encoding.
* serial_type_length() translates that encoding into the number of bytes to consume.
fn serial_type_length(serial_type: u64) -> io::Result<usize> {
*/
fn serial_type_length(serial_type: u64) -> io::Result<usize> {
    match serial_type {
        0 | 8 | 9 => Ok(0),
        1 => Ok(1),
        2 => Ok(2),
        3 => Ok(3),
        4 => Ok(4),
        5 => Ok(6),
        6 | 7 => Ok(8),
        10 | 11 => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "reserved SQLite serial type",
        )),
        n if n >= 12 && n % 2 == 0 => Ok(((n - 12) / 2) as usize),
        n if n >= 13 && n % 2 == 1 => Ok(((n - 13) / 2) as usize),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid SQLite serial type",
        )),
    }
}
