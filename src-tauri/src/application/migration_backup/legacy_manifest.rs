//! Reads one legacy JSON manifest entry per turn without loading the array.
use super::*;
use std::io::{BufReader, Seek, SeekFrom};

pub(super) fn next(path: &Path, offset: u64) -> Result<Option<(u64, PathBuf, Option<String>)>> {
    let parent = path
        .parent()
        .ok_or_else(|| GardenError::Recovery("Legacy manifest has no parent directory.".into()))?;
    let name = path
        .file_name()
        .ok_or_else(|| GardenError::Recovery("Legacy manifest has no file name.".into()))?;
    let file = bounded_copy::source_file(parent, Path::new(name))?;
    let mut input = BufReader::new(file);
    input.seek(SeekFrom::Start(offset))?;
    fn byte(input: &mut BufReader<File>) -> Result<Option<u8>> {
        let mut value = [0];
        Ok(if input.read(&mut value)? == 0 {
            None
        } else {
            Some(value[0])
        })
    }
    if offset == 0 && byte(&mut input)? != Some(b'[') {
        return Err(GardenError::Recovery(
            "Legacy manifest is not an array.".into(),
        ));
    }
    fn nonspace(input: &mut BufReader<File>) -> Result<Option<u8>> {
        for _ in 0..16_384 {
            match byte(input)? {
                Some(b' ' | b'\n' | b'\r' | b'\t') => (),
                value => return Ok(value),
            }
        }
        Err(GardenError::Recovery(
            "Legacy manifest has excessive whitespace.".into(),
        ))
    }
    let first = nonspace(&mut input)?;
    let item = if offset == 0 {
        first
    } else {
        match first {
            Some(b',') => nonspace(&mut input)?,
            Some(b']') => Some(b']'),
            _ => {
                return Err(GardenError::Recovery(
                    "Legacy manifest separator is invalid.".into(),
                ))
            }
        }
    };
    let start = match item {
        Some(b']') if offset == 0 || first == Some(b']') => {
            if nonspace(&mut input)?.is_some() {
                return Err(GardenError::Recovery(
                    "Legacy manifest has trailing data.".into(),
                ));
            }
            return Ok(None);
        }
        Some(b'[') => b'[',
        _ => {
            return Err(GardenError::Recovery(
                "Legacy manifest is malformed.".into(),
            ))
        }
    };
    let mut bytes = vec![start];
    let mut depth = 1usize;
    let mut string = false;
    let mut escaped = false;
    while depth > 0 {
        if bytes.len() >= 16_384 {
            return Err(GardenError::Recovery(
                "Legacy manifest entry is too large.".into(),
            ));
        }
        let value = byte(&mut input)?
            .ok_or_else(|| GardenError::Recovery("Legacy manifest is truncated.".into()))?;
        bytes.push(value);
        if string {
            if escaped {
                escaped = false;
            } else if value == b'\\' {
                escaped = true;
            } else if value == b'"' {
                string = false;
            }
        } else if value == b'"' {
            string = true;
        } else if value == b'[' {
            depth += 1;
        } else if value == b']' {
            depth -= 1;
        }
    }
    let (relative, digest): (PathBuf, Option<String>) = serde_json::from_slice(&bytes)?;
    validate_relative_collection_path(&relative)?;
    if let Some(value) = &digest {
        if value.len() != 64
            || !value
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(GardenError::Recovery(
                "Legacy manifest digest is invalid.".into(),
            ));
        }
    }
    Ok(Some((input.stream_position()?, relative, digest)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_one_entry_at_a_time_and_rejects_escaped_paths() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("manifest.json");
        fs::write(
            &path,
            serde_json::to_vec(&vec![
                (PathBuf::from("dir"), None),
                (PathBuf::from("dir/file"), Some("a".repeat(64))),
            ])
            .unwrap(),
        )
        .unwrap();
        let (offset, name, digest) = next(&path, 0).unwrap().unwrap();
        assert_eq!(name, PathBuf::from("dir"));
        assert_eq!(digest, None);
        let (offset, name, _) = next(&path, offset).unwrap().unwrap();
        assert_eq!(name, PathBuf::from("dir/file"));
        assert!(next(&path, offset).unwrap().is_none());
        fs::write(&path, b"[[\"../escape\",null]]").unwrap();
        assert!(next(&path, 0).is_err());
    }
}
