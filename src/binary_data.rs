use std::io::{Cursor, Read};
pub fn read_bytes(reader: &mut Cursor<Vec<u8>>, length: usize) -> anyhow::Result<Vec<u8>> {
    let mut buffer = vec![0u8; length];
    reader.read_exact(&mut buffer)?;
    return Ok(buffer);
}

pub fn read_string(reader: &mut Cursor<Vec<u8>>, length: usize) -> anyhow::Result<String> {
    return Ok(String::from_utf8(read_bytes(reader, length)?)?);
}
