//! Bounded, byte-addressed UTF-8 reads for retained source versions.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub struct TextChunk {
    pub start: u64,
    pub end: u64,
    pub text: String,
    pub is_last: bool,
}

/// Read at most `limit` bytes, ending on a UTF-8 character boundary.
/// `start` must be the end returned by a previous read (or zero).
pub fn read_chunk(path: &Path, start: u64, limit: usize) -> io::Result<Option<TextChunk>> {
    if limit < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "chunk limit must be at least four bytes",
        ));
    }
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if start > size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "chunk offset exceeds source length",
        ));
    }
    if start == size {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(limit);
    file.take(limit as u64).read_to_end(&mut bytes)?;
    let end = match std::str::from_utf8(&bytes) {
        Ok(_) => bytes.len(),
        Err(error) if error.error_len().is_none() && start + (bytes.len() as u64) < size => {
            error.valid_up_to()
        }
        Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidData, error)),
    };
    bytes.truncate(end);
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "text contains binary control characters",
        ));
    }
    let end = start + end as u64;
    Ok(Some(TextChunk {
        start,
        end,
        text,
        is_last: end == size,
    }))
}

/// Search an original without loading the original into memory. The pattern is
/// source evidence and is bounded separately by the staged record size limit.
pub fn contains_bytes(path: &Path, needle: &[u8]) -> io::Result<bool> {
    if needle.is_empty() {
        return Ok(true);
    }
    let mut prefix = vec![0; needle.len()];
    let mut matched = 0;
    for i in 1..needle.len() {
        while matched > 0 && needle[i] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[i] == needle[matched] {
            matched += 1;
        }
        prefix[i] = matched;
    }
    let mut file = File::open(path)?;
    let mut buffer = [0; 64 * 1024];
    matched = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(false);
        }
        for &byte in &buffer[..count] {
            while matched > 0 && byte != needle[matched] {
                matched = prefix[matched - 1];
            }
            if byte == needle[matched] {
                matched += 1;
                if matched == needle.len() {
                    return Ok(true);
                }
            }
        }
    }
}

/// Include a small preceding UTF-8 context without exceeding the provider limit.
/// The caller commits the new range starting at `offset`, not the overlap.
pub fn read_window(
    path: &Path,
    offset: u64,
    limit: usize,
    overlap: u64,
) -> io::Result<Option<TextChunk>> {
    if overlap >= limit as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "overlap must be smaller than the window",
        ));
    }
    let mut file = File::open(path)?;
    if offset == file.metadata()?.len() {
        return Ok(None);
    }
    if offset > file.metadata()?.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "window offset exceeds the source",
        ));
    }
    let mut start = offset.saturating_sub(overlap);
    file.seek(SeekFrom::Start(start))?;
    let mut byte = [0];
    while start < offset && file.read(&mut byte)? == 1 && byte[0] & 0xc0 == 0x80 {
        start += 1;
    }
    let chunk = read_chunk(path, start, limit)?;
    if chunk.as_ref().is_some_and(|chunk| chunk.end <= offset) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "text window made no progress",
        ));
    }
    Ok(chunk)
}
