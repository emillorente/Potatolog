use std::fs;
use std::io::{BufRead, BufReader, Error as IoError, Read, Seek, SeekFrom};
use std::path::Path;

pub trait LogReader {
    fn seek(&mut self, pos: u64) -> Result<(), IoError>;
    fn tell(&self) -> u64;
    fn read_record(&mut self) -> Result<Option<String>, IoError>;
}

pub struct LogFile {
    file: BufReader<fs::File>,
    pos: u64,
}

impl LogFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<LogFile, IoError> {
        Ok(LogFile {
            file: BufReader::new(fs::File::open(path)?),
            pos: 0,
        })
    }
}

impl LogReader for LogFile {
    fn seek(&mut self, pos: u64) -> Result<(), IoError> {
        self.file.seek(SeekFrom::Start(pos))?;
        self.pos = pos;
        Ok(())
    }

    fn tell(&self) -> u64 {
        self.pos
    }

    fn read_record(&mut self) -> Result<Option<String>, IoError> {
        let mut line = String::new();
        let ret = self.file.read_line(&mut line)?;
        if ret == 0 {
            Ok(None)
        } else {
            self.pos += ret as u64;
            if line.len() >= 2 && line.ends_with("\r\n") {
                line.pop();
                line.pop();
            } else if !line.is_empty() && line.ends_with("\n") {
                line.pop();
            }
            Ok(Some(line))
        }
    }
}

/// Reader for CORE.OUT format: records are delimited by `~@_~` (not by newlines).
/// This handles multi-line XML messages that span many physical lines.
pub struct LogCoreReader {
    file: BufReader<fs::File>,
    pos: u64,
}

impl LogCoreReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<LogCoreReader, IoError> {
        Ok(LogCoreReader {
            file: BufReader::new(fs::File::open(path)?),
            pos: 0,
        })
    }
}

impl LogReader for LogCoreReader {
    fn seek(&mut self, pos: u64) -> Result<(), IoError> {
        self.file.seek(SeekFrom::Start(pos))?;
        self.pos = pos;
        Ok(())
    }

    fn tell(&self) -> u64 {
        self.pos
    }

    fn read_record(&mut self) -> Result<Option<String>, IoError> {
        let mut record = String::with_capacity(512);
        let mut state: u8 = 0;
        let mut started = false;

        loop {
            let buf = self.file.fill_buf()?;
            if buf.is_empty() {
                if record.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(record));
            }
            let mut consumed = 0;
            for &byte in buf {
                consumed += 1;
                self.pos += 1;
                let c = byte as char;
                if !started && (c == '\n' || c == '\r' || c == ' ' || c == '\t') {
                    continue;
                }
                started = true;
                record.push(c);

                match state {
                    0 if c == '~' => state = 1,
                    1 if c == '@' => state = 2,
                    2 if c == '_' => state = 3,
                    3 if c == '~' => {
                        self.file.consume(consumed);
                        return Ok(Some(record));
                    }
                    _ => state = if c == '~' { 1 } else { 0 },
                }
            }
            self.file.consume(consumed);
        }
    }
}

pub struct LogQueryReader {
    file: BufReader<fs::File>,
    pos: u64,
}

impl LogQueryReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<LogQueryReader, IoError> {
        Ok(LogQueryReader {
            file: BufReader::new(fs::File::open(path)?),
            pos: 0,
        })
    }

    fn read_line_trim(&mut self) -> Result<Option<String>, IoError> {
        let mut line = String::new();
        let ret = self.file.read_line(&mut line)?;
        if ret == 0 {
            return Ok(None);
        }
        self.pos += ret as u64;
        if line.len() >= 2 && line.ends_with("\r\n") { line.pop(); line.pop(); }
        else if !line.is_empty() && line.ends_with("\n") { line.pop(); }
        Ok(Some(line))
    }
}

impl LogReader for LogQueryReader {
    fn seek(&mut self, pos: u64) -> Result<(), IoError> {
        self.file.seek(SeekFrom::Start(pos))?;
        self.pos = pos;
        Ok(())
    }

    fn tell(&self) -> u64 {
        self.pos
    }

    fn read_record(&mut self) -> Result<Option<String>, IoError> {
        let header = loop {
            match self.read_line_trim()? {
                None => return Ok(None),
                Some(line) if !line.is_empty() && !line.starts_with("/***") => break line,
                _ => continue,
            }
        };

        let sql = loop {
            match self.read_line_trim()? {
                None => break String::new(),
                Some(line) if !line.is_empty() && !line.eq_ignore_ascii_case("go") => break line,
                _ => continue,
            }
        };

        // Skip trailing "go" + blank (reu format: sql + go + blank + next header)
        let _ = self.read_line_trim()?; // skip "go" or whatever follows
        let _ = self.read_line_trim()?; // skip blank after go or whatever follows

        Ok(Some(header + "~" + &sql))
    }
}

pub fn detect_reader<P: AsRef<Path>>(path: P) -> Result<Box<dyn LogReader>, IoError> {
    let path = path.as_ref();
    let mut file = BufReader::new(fs::File::open(path)?);
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf)?;
    let head = String::from_utf8_lossy(&buf[..n]);

    drop(file);
    if head.trim_start().starts_with("/***") {
        Ok(Box::new(LogQueryReader::open(path)?))
    } else if head.contains("~@_~") {
        Ok(Box::new(LogCoreReader::open(path)?))
    } else {
        Ok(Box::new(LogFile::open(path)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn read_all(mut reader: Box<dyn LogReader>) -> Vec<String> {
        let mut records = Vec::new();
        while let Some(rec) = reader.read_record().unwrap() {
            records.push(rec);
        }
        records
    }

    #[test]
    fn log_core_reader_reads_delimited_records() {
        let reader = LogCoreReader::open(fixture("sample_core.out")).unwrap();
        let records = read_all(Box::new(reader));
        assert_eq!(records.len(), 2);
        assert!(records[0].contains("~alice~"));
        assert!(records[0].ends_with("~@_~"));
        assert!(records[1].contains("~bob~"));
    }

    #[test]
    fn log_query_reader_merges_header_and_sql() {
        let reader = LogQueryReader::open(fixture("sample_reu.out")).unwrap();
        let records = read_all(Box::new(reader));
        assert_eq!(records.len(), 2);
        assert!(records[0].contains("SELECT * FROM users"));
        assert!(records[0].contains("CONTEXT@"));
        assert!(records[1].contains("INSERT INTO logs"));
    }

    #[test]
    fn log_file_reads_line_by_line() {
        let reader = LogFile::open(fixture("sample_plain.log")).unwrap();
        let records = read_all(Box::new(reader));
        assert_eq!(records, vec!["line one", "line two", "line three"]);
    }

    #[test]
    fn detect_reader_picks_core_format() {
        let path = fixture("sample_core.out");
        let reader = detect_reader(&path).unwrap();
        let records = read_all(reader);
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn detect_reader_picks_reu_format() {
        let path = fixture("sample_reu_detect.out");
        let reader = detect_reader(&path).unwrap();
        let records = read_all(reader);
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn detect_reader_picks_plain_format() {
        let path = fixture("sample_plain.log");
        let reader = detect_reader(&path).unwrap();
        let records = read_all(reader);
        assert_eq!(records.len(), 3);
    }
}
