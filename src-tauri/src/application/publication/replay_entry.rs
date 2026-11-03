//! Decode one framed legacy entry without collecting its embedded base bytes.
use super::*;
use sha2::Digest as ShaDigest;
use std::io::BufReader;

const BUDGET: usize = 64 * 1024;
const MAX_STRING: usize = 128 * 1024;
#[derive(Serialize, Deserialize)]
pub(super) struct Meta {
    pub destination: String,
    pub staged: String,
    pub base_sha256: Option<String>,
    pub base_length: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    KeyFirst,
    KeyRequired,
    Key,
    Colon,
    Value,
    StringValue,
    Null,
    ArrayFirst,
    ArrayRequired,
    Number,
    AfterValue,
    Tail,
    Done,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Destination,
    Staged,
    Expected,
}
pub(super) struct Decoder {
    input: BufReader<File>,
    root: PathBuf,
    work: PathBuf,
    index: u64,
    phase: Phase,
    field: Option<Field>,
    literal: Vec<u8>,
    escaped: bool,
    destination: Option<String>,
    staged: Option<String>,
    expected_seen: bool,
    null_pos: usize,
    number: u16,
    digits: u8,
    output: Option<super::replay_manifest::ScratchOutput>,
    hasher: Option<Sha256>,
    base_length: u64,
    base_sha256: Option<String>,
    mac: Option<super::replay_manifest::Mac>,
    expected_tag: Vec<u8>,
}
impl Decoder {
    pub(super) fn new(
        root: &Path,
        work: &Path,
        index: u64,
        key: [u8; 32],
        expected_tag: Vec<u8>,
    ) -> Result<Self> {
        let input =
            migration_backup::open_nofollow(root, &work.join("entries").join(index.to_string()))?;
        Ok(Self {
            input: BufReader::new(input),
            root: root.into(),
            work: work.into(),
            index,
            phase: Phase::Start,
            field: None,
            literal: Vec::new(),
            escaped: false,
            destination: None,
            staged: None,
            expected_seen: false,
            null_pos: 0,
            number: 0,
            digits: 0,
            output: None,
            hasher: None,
            base_length: 0,
            base_sha256: None,
            mac: Some(super::replay_manifest::Mac::new(&key, "entries", index)),
            expected_tag,
        })
    }
    fn begin_string(&mut self) {
        self.literal.clear();
        self.literal.push(b'"');
        self.escaped = false;
    }
    fn string_byte(&mut self, byte: u8) -> Result<Option<String>> {
        if self.literal.len() >= MAX_STRING {
            return Err(GardenError::Recovery(
                "Publication entry string is too large.".into(),
            ));
        }
        self.literal.push(byte);
        if self.escaped {
            self.escaped = false;
            return Ok(None);
        }
        if byte == b'\\' {
            self.escaped = true;
            return Ok(None);
        }
        if byte == b'"' {
            let value = serde_json::from_slice::<String>(&self.literal)?;
            return Ok(Some(value));
        }
        Ok(None)
    }
    fn base_byte(&mut self) -> Result<()> {
        if self.digits == 0 || self.number > 255 {
            return Err(GardenError::Recovery(
                "Publication base byte is invalid.".into(),
            ));
        }
        let byte = self.number as u8;
        self.output.as_mut().unwrap().push(byte);
        self.hasher.as_mut().unwrap().update([byte]);
        self.base_length = self
            .base_length
            .checked_add(1)
            .ok_or_else(|| GardenError::Recovery("Publication base is too large.".into()))?;
        self.number = 0;
        self.digits = 0;
        Ok(())
    }
    fn finish_base(&mut self) -> Result<()> {
        self.output.take().unwrap().finish()?;
        self.base_sha256 = Some(hex::encode(self.hasher.take().unwrap().finalize()));
        self.phase = Phase::AfterValue;
        Ok(())
    }
    pub(super) fn step(&mut self) -> Result<Option<Meta>> {
        for _ in 0..BUDGET {
            let mut buf = [0];
            let count = self.input.read(&mut buf)?;
            if count == 0 {
                if self.phase != Phase::Tail {
                    return Err(GardenError::Recovery(
                        "Publication entry is truncated.".into(),
                    ));
                }
                self.phase = Phase::Done;
                if self.expected_tag != self.mac.take().unwrap().finish() {
                    return Err(GardenError::Recovery(
                        "Framed publication entry changed after verification.".into(),
                    ));
                }
                let destination = self.destination.take().ok_or_else(|| {
                    GardenError::Recovery("Publication destination is missing.".into())
                })?;
                let staged = self.staged.take().ok_or_else(|| {
                    GardenError::Recovery("Publication staging path is missing.".into())
                })?;
                validate_relative_collection_path(Path::new(&destination))?;
                validate_relative_staging_path(Path::new(&staged))?;
                return Ok(Some(Meta {
                    destination,
                    staged,
                    base_sha256: self.base_sha256.take(),
                    base_length: self.base_length,
                }));
            }
            let b = buf[0];
            self.mac.as_mut().unwrap().update(&[b]);
            match self.phase {
                Phase::Start => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if b != b'{' {
                        return Err(GardenError::Recovery(
                            "Publication entry is not an object.".into(),
                        ));
                    }
                    self.phase = Phase::KeyFirst;
                }
                Phase::KeyFirst | Phase::KeyRequired => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if b == b'}' && self.phase == Phase::KeyFirst {
                        self.phase = Phase::Tail;
                    } else if b == b'"' {
                        self.begin_string();
                        self.phase = Phase::Key;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication entry key is invalid.".into(),
                        ));
                    }
                }
                Phase::Key => {
                    if let Some(key) = self.string_byte(b)? {
                        self.field = Some(match key.as_str() {
                            "destination" => Field::Destination,
                            "staged" => Field::Staged,
                            "expected" => Field::Expected,
                            _ => {
                                return Err(GardenError::Recovery(
                                    "Publication entry contains an unknown field.".into(),
                                ))
                            }
                        });
                        self.phase = Phase::Colon;
                    }
                }
                Phase::Colon => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if b != b':' {
                        return Err(GardenError::Recovery(
                            "Publication entry field is invalid.".into(),
                        ));
                    }
                    self.phase = Phase::Value;
                }
                Phase::Value => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    match self.field.unwrap() {
                        Field::Destination | Field::Staged => {
                            if b != b'"' {
                                return Err(GardenError::Recovery(
                                    "Publication entry path is invalid.".into(),
                                ));
                            }
                            self.begin_string();
                            self.phase = Phase::StringValue;
                        }
                        Field::Expected => {
                            if self.expected_seen {
                                return Err(GardenError::Recovery(
                                    "Publication entry repeats its base.".into(),
                                ));
                            }
                            self.expected_seen = true;
                            if b == b'n' {
                                self.null_pos = 0;
                                self.phase = Phase::Null;
                            } else if b == b'[' {
                                let path = self.work.join("bases").join(self.index.to_string());
                                self.output = Some(super::replay_manifest::ScratchOutput::open(
                                    &self.root, &path,
                                )?);
                                self.hasher = Some(Sha256::new());
                                self.phase = Phase::ArrayFirst;
                            } else {
                                return Err(GardenError::Recovery(
                                    "Publication entry base is invalid.".into(),
                                ));
                            }
                        }
                    }
                }
                Phase::StringValue => {
                    if let Some(value) = self.string_byte(b)? {
                        if value.len() > 16384 {
                            return Err(GardenError::Recovery(
                                "Publication entry path is too large.".into(),
                            ));
                        }
                        match self.field.unwrap() {
                            Field::Destination if self.destination.is_none() => {
                                self.destination = Some(value)
                            }
                            Field::Staged if self.staged.is_none() => self.staged = Some(value),
                            _ => {
                                return Err(GardenError::Recovery(
                                    "Publication entry repeats a path.".into(),
                                ))
                            }
                        }
                        self.phase = Phase::AfterValue;
                    }
                }
                Phase::Null => {
                    let expected = b"ull";
                    if expected.get(self.null_pos) != Some(&b) {
                        return Err(GardenError::Recovery(
                            "Publication entry base is invalid.".into(),
                        ));
                    }
                    self.null_pos += 1;
                    if self.null_pos == expected.len() {
                        self.phase = Phase::AfterValue;
                    }
                }
                Phase::ArrayFirst | Phase::ArrayRequired => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if b == b']' && self.phase == Phase::ArrayFirst {
                        self.finish_base()?;
                    } else if b.is_ascii_digit() {
                        self.number = (b - b'0') as u16;
                        self.digits = 1;
                        self.phase = Phase::Number;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication base array is invalid.".into(),
                        ));
                    }
                }
                Phase::Number => {
                    if b.is_ascii_digit() {
                        if self.digits >= 3 || (self.digits == 1 && self.number == 0) {
                            return Err(GardenError::Recovery(
                                "Publication base byte is invalid.".into(),
                            ));
                        }
                        self.number = self.number * 10 + (b - b'0') as u16;
                        self.digits += 1;
                    } else {
                        self.base_byte()?;
                        if b == b',' {
                            self.phase = Phase::ArrayRequired;
                        } else if b == b']' {
                            self.finish_base()?;
                        } else if b.is_ascii_whitespace() {
                            self.phase = Phase::AfterValue;
                            self.field = Some(Field::Expected);
                            self.null_pos = 99;
                        } else {
                            return Err(GardenError::Recovery(
                                "Publication base separator is invalid.".into(),
                            ));
                        }
                    }
                }
                Phase::AfterValue => {
                    if b.is_ascii_whitespace() {
                        continue;
                    }
                    if self.null_pos == 99 {
                        self.null_pos = 0;
                        if b == b',' {
                            self.phase = Phase::ArrayRequired;
                        } else if b == b']' {
                            self.finish_base()?;
                        } else {
                            return Err(GardenError::Recovery(
                                "Publication base separator is invalid.".into(),
                            ));
                        }
                    } else if b == b',' {
                        self.phase = Phase::KeyRequired;
                    } else if b == b'}' {
                        self.phase = Phase::Tail;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication entry separator is invalid.".into(),
                        ));
                    }
                }
                Phase::Tail => {
                    if !b.is_ascii_whitespace() {
                        return Err(GardenError::Recovery(
                            "Publication entry has trailing data.".into(),
                        ));
                    }
                }
                Phase::Done => {
                    return Err(GardenError::Recovery(
                        "Publication entry was already decoded.".into(),
                    ))
                }
            }
        }
        if let Some(output) = self.output.as_mut() {
            output.flush()?;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_base_decodes_in_bounded_turns_and_resumes() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let work = Path::new("scratch");
        fs::create_dir_all(root.join(work).join("entries")).unwrap();
        fs::create_dir_all(root.join(work).join("bases")).unwrap();
        let entry = PublicationEntry {
            destination: "notes/page".into(),
            staged: "files/0000".into(),
            expected: Some(vec![10; 250_000]),
        };
        let raw = serde_json::to_vec(&entry).unwrap();
        fs::write(root.join(work).join("entries/0"), &raw).unwrap();
        let key = [3u8; 32];
        let mut mac = super::replay_manifest::Mac::new(&key, "entries", 0);
        mac.update(&raw);
        let tag = mac.finish().to_vec();
        let mut first = Decoder::new(&root, work, 0, key, tag.clone()).unwrap();
        assert!(first.step().unwrap().is_none());
        drop(first);
        let mut resumed = Decoder::new(&root, work, 0, key, tag).unwrap();
        let mut turns = 0;
        let result = loop {
            turns += 1;
            if let Some(value) = resumed.step().unwrap() {
                break value;
            }
            assert!(turns < 100);
        };
        assert!(turns > 1);
        assert_eq!(result.base_length, 250_000);
        assert_eq!(
            result.base_sha256.unwrap(),
            hex::encode(Sha256::digest(vec![10; 250_000]))
        );
        assert_eq!(
            fs::read(root.join(work).join("bases/0")).unwrap(),
            vec![10; 250_000]
        );
    }
}
