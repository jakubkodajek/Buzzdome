//! Whether a ZIP file counts more entries than a quiz may have, read before
//! the archive is opened: opening reads every entry header the end record
//! it takes counts, so a file of millions of empty entries would take all
//! memory before `MAX_ENTRIES` could be checked.
//!
//! A plain end record counts at most 65,535 entries, which opening reads at
//! a bounded cost and the count after it refuses. A ZIP64 end record can
//! count billions. The reader takes the last end record of the file that
//! reads and goes further back when one does not (`zip` 9:
//! `find_central_directory_end`, `ZipArchive::get_metadata`), so every
//! ZIP64 end record in the file is checked, not only the last.

use memchr::memmem;
use std::collections::HashSet;
use std::io::{self, Read, Seek, SeekFrom};

/// How the signatures of the ZIP64 end record and its locator start; the
/// fourth byte tells them apart.
const SIGNATURE: &[u8; 3] = b"PK\x06";
const ZIP64_END: u8 = 0x06;
const ZIP64_LOCATOR: u8 = 0x07;
const END: &[u8; 4] = b"PK\x05\x06";
/// From the start of a ZIP64 end record to the end of its entry counts.
const ZIP64_END_COUNTS: usize = 40;
/// What a ZIP64 end record's size leaves out: its signature and the size.
const ZIP64_END_OVERHEAD: u64 = 12;
const ZIP64_LOCATOR_LEN: usize = 20;
/// How many ZIP64 end records counting too many a file may hold before it
/// is refused: a quiz file holds one at most and chance matches in media
/// are rare, so more is a file made to be checked slowly.
const MAX_SUSPECTS: usize = 64;
/// How much of the file is read at a time.
const CHUNK: usize = 1 << 20;

/// Whether `file` holds a ZIP64 end record counting more than `limit`
/// entries that a ZIP reader could take: one followed, where its size says
/// it ends, by its locator and then an end record, which is how the reader
/// finds it.
pub fn counts_more_than(file: &mut (impl Read + Seek), limit: u64) -> io::Result<bool> {
    scan(file, limit, CHUNK)
}

fn scan(file: &mut (impl Read + Seek), limit: u64, chunk_len: usize) -> io::Result<bool> {
    // Where the ZIP64 end records counting too many say they end.
    let mut suspects = HashSet::new();
    let mut chunk = Vec::with_capacity(chunk_len + ZIP64_END_COUNTS);
    // The file offset of `chunk[0]`.
    let mut start = 0;
    file.seek(SeekFrom::Start(0))?;
    loop {
        let kept = chunk.len();
        file.by_ref().take(chunk_len as u64).read_to_end(&mut chunk)?;
        let last = chunk.len() - kept < chunk_len;
        // A record is read where all its fields are in this chunk; the
        // bytes after `done` come again at the start of the next one.
        let done = if last { chunk.len() } else { chunk.len() - ZIP64_END_COUNTS };
        for at in memmem::find_iter(&chunk, SIGNATURE).take_while(|&at| at < done) {
            let offset = start + at as u64;
            match chunk.get(at + 3) {
                Some(&ZIP64_END) => {
                    let Some(record) = chunk.get(at..at + ZIP64_END_COUNTS) else { continue };
                    if u64_at(record, 24).max(u64_at(record, 32)) <= limit {
                        continue;
                    }
                    if let Some(end) = offset.checked_add(u64_at(record, 4)).and_then(|end| end.checked_add(ZIP64_END_OVERHEAD)) {
                        suspects.insert(end);
                    }
                    if suspects.len() > MAX_SUSPECTS {
                        return Ok(true);
                    }
                }
                Some(&ZIP64_LOCATOR) => {
                    let followed_by_end = chunk.get(at + ZIP64_LOCATOR_LEN..).is_some_and(|after| after.starts_with(END));
                    if followed_by_end && suspects.contains(&offset) {
                        return Ok(true);
                    }
                }
                _ => {}
            }
        }
        if last {
            return Ok(false);
        }
        chunk.drain(..done);
        start += done as u64;
    }
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    /// A ZIP64 end record counting `count` entries at offset `at`, its
    /// locator and an end record that defers to it.
    pub(crate) fn zip64_end(count: u64, at: u64) -> Vec<u8> {
        let mut record = [0u8; 56];
        record[..4].copy_from_slice(b"PK\x06\x06");
        record[4..12].copy_from_slice(&44u64.to_le_bytes());
        record[24..32].copy_from_slice(&count.to_le_bytes());
        record[32..40].copy_from_slice(&count.to_le_bytes());
        let mut locator = [0u8; ZIP64_LOCATOR_LEN];
        locator[..4].copy_from_slice(b"PK\x06\x07");
        locator[8..16].copy_from_slice(&at.to_le_bytes());
        locator[16..20].copy_from_slice(&1u32.to_le_bytes());
        let mut end = [0u8; 22];
        end[..4].copy_from_slice(END);
        end[10..12].copy_from_slice(&u16::MAX.to_le_bytes());
        [&record[..], &locator, &end].concat()
    }

    fn counts_too_many(bytes: Vec<u8>, chunk_len: usize) -> bool {
        scan(&mut Cursor::new(bytes), 10_000, chunk_len).unwrap()
    }

    #[test]
    fn a_plain_zip_file_counts_nothing_too_many() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for n in 0..3 {
            zip.start_file(format!("media/{n}.png"), SimpleFileOptions::default()).unwrap();
            zip.write_all(b"x").unwrap();
        }
        zip.set_comment("PK\x06\x06 and PK\x06\x07 in a comment").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert!(!counts_too_many(bytes, CHUNK));
        assert!(!counts_too_many(Vec::new(), CHUNK));
    }

    #[test]
    fn finds_a_zip64_end_record_counting_too_many_wherever_it_is() {
        // Anywhere in the file, also across the boundary of two chunks.
        for at in 0..150 {
            let bytes = |count| [vec![b'.'; at], zip64_end(count, at as u64), vec![b'.'; 30]].concat();
            assert!(counts_too_many(bytes(3_000_000), 64), "at {at}");
            assert!(!counts_too_many(bytes(10_000), 64), "at {at}");
        }
        // As the last thing in the file.
        assert!(counts_too_many(zip64_end(3_000_000, 0), CHUNK));
    }

    #[test]
    fn a_signature_by_chance_is_not_a_record() {
        // A ZIP64 end record whose locator is not where its size says.
        let mut bytes = zip64_end(3_000_000, 0);
        bytes[4..12].copy_from_slice(&45u64.to_le_bytes());
        assert!(!counts_too_many(bytes, CHUNK));
        // A locator followed by no end record.
        let mut bytes = zip64_end(3_000_000, 0);
        bytes[76..80].copy_from_slice(b"....");
        assert!(!counts_too_many(bytes, CHUNK));
    }

    #[test]
    fn a_file_of_many_such_records_is_refused_unchecked() {
        // Records that each say they end somewhere else, so none is followed by its locator.
        let records = |n| {
            let mut bytes = Vec::new();
            for _ in 0..n {
                let size_at = bytes.len() + 4;
                bytes.extend(zip64_end(3_000_000, 0));
                let elsewhere = bytes.len() as u64;
                bytes[size_at..size_at + 8].copy_from_slice(&elsewhere.to_le_bytes());
            }
            bytes
        };
        assert!(!counts_too_many(records(MAX_SUSPECTS), CHUNK));
        assert!(counts_too_many(records(MAX_SUSPECTS + 1), CHUNK));
    }
}
