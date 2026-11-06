//! The neural voice model the app reads questions with: where it comes
//! from, how its download is checked, and how it is unpacked. The model is
//! not shipped with the app: the quizmaster downloads it once (about
//! 129 MB), and from then on it works without a network.

use crate::VoiceError;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The voice model: Supertonic 3 (Supertone, OpenRAIL-M licence, 31
/// languages), quantized to 8 bits and packed for sherpa-onnx by its authors.
pub struct ModelSource {
    /// Names the model's folder, and goes into every clip's cache key.
    pub id: &'static str,
    pub url: &'static str,
    /// SHA-256 of the archive, so that a damaged or changed download is refused.
    pub sha256: &'static str,
    pub bytes: u64,
    /// How many voices it has, numbered from 0.
    pub voices: u8,
}

pub const MODEL: ModelSource = ModelSource {
    id: "sherpa-onnx-supertonic-3-tts-int8-2026-05-11",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2",
    sha256: "82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427",
    bytes: 128_774_318,
    voices: 10,
};

/// The files the engine loads from the model's folder.
pub const MODEL_FILES: [&str; 7] = [
    "duration_predictor.int8.onnx",
    "text_encoder.int8.onnx",
    "vector_estimator.int8.onnx",
    "vocoder.int8.onnx",
    "tts.json",
    "unicode_indexer.bin",
    "voice.bin",
];

/// Written last when a model is unpacked: a folder without it is not used.
const INSTALLED_MARK: &str = "installed";

/// The model's folder under `root`, the app's voice folder.
pub fn model_dir(root: &Path) -> PathBuf {
    root.join(MODEL.id)
}

/// Whether the model under `root` was unpacked whole.
pub fn installed(root: &Path) -> bool {
    let dir = model_dir(root);
    dir.join(INSTALLED_MARK).is_file() && MODEL_FILES.iter().all(|file| dir.join(file).is_file())
}

/// Copies `from` into `to`, hashing what passes; `progress` hears the bytes
/// so far. Fails once more than `MODEL.bytes` arrive.
pub fn receive(mut from: impl Read, mut to: impl Write, mut progress: impl FnMut(u64)) -> Result<String, VoiceError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut total = 0u64;
    loop {
        let n = from.read(&mut buffer).map_err(VoiceError::download)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MODEL.bytes {
            return Err(VoiceError::Damaged);
        }
        hasher.update(&buffer[..n]);
        to.write_all(&buffer[..n]).map_err(VoiceError::disk)?;
        progress(total);
    }
    to.flush().map_err(VoiceError::disk)?;
    Ok(hex(&hasher.finalize()))
}

/// Unpacks the downloaded `archive`, whose hash `receive` gave as `sha256`,
/// into the model's folder under `root`. Only the model's own files are
/// taken; the folder is replaced as a whole, so a failed unpack leaves none.
pub fn unpack(archive: &Path, sha256: &str, root: &Path) -> Result<(), VoiceError> {
    if sha256 != MODEL.sha256 {
        return Err(VoiceError::Damaged);
    }
    let staging = root.join(format!("{}.unpacking", MODEL.id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(VoiceError::disk)?;
    let unpacked = unpack_into(archive, &staging);
    if let Err(e) = unpacked {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    let dir = model_dir(root);
    let _ = fs::remove_dir_all(&dir);
    fs::rename(&staging, &dir).map_err(VoiceError::disk)
}

fn unpack_into(archive: &Path, staging: &Path) -> Result<(), VoiceError> {
    let file = fs::File::open(archive).map_err(VoiceError::disk)?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(io::BufReader::new(file)));
    for entry in tar.entries().map_err(|_| VoiceError::Damaged)? {
        let mut entry = entry.map_err(|_| VoiceError::Damaged)?;
        let path = entry.path().map_err(|_| VoiceError::Damaged)?.into_owned();
        // Only `<id>/<file>` for a file the engine loads: nothing else is written.
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        let parent = path.parent().and_then(|parent| parent.to_str()).unwrap_or_default();
        if parent != MODEL.id || !MODEL_FILES.contains(&name) {
            continue;
        }
        let mut out = fs::File::create(staging.join(name)).map_err(VoiceError::disk)?;
        io::copy(&mut entry, &mut out).map_err(|_| VoiceError::Damaged)?;
        out.sync_all().map_err(VoiceError::disk)?;
    }
    if !MODEL_FILES.iter().all(|file| staging.join(file).is_file()) {
        return Err(VoiceError::Damaged);
    }
    fs::write(staging.join(INSTALLED_MARK), MODEL.sha256).map_err(VoiceError::disk)
}

/// Removes the model under `root`, and anything left of an unpack.
pub fn remove(root: &Path) -> Result<(), VoiceError> {
    let _ = fs::remove_dir_all(root.join(format!("{}.unpacking", MODEL.id)));
    match fs::remove_dir_all(model_dir(root)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(VoiceError::disk(e)),
        _ => Ok(()),
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A .tar.bz2 archive with `files` (path, contents).
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, path, *contents).unwrap();
        }
        let mut packed = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        packed.write_all(&tar.into_inner().unwrap()).unwrap();
        packed.finish().unwrap()
    }

    fn model_archive(extra: &[(&str, &[u8])]) -> Vec<u8> {
        let mut files: Vec<(String, &[u8])> = MODEL_FILES.iter().map(|file| (format!("{}/{file}", MODEL.id), b"x" as &[u8])).collect();
        files.extend(extra.iter().map(|(path, contents)| (path.to_string(), *contents)));
        let files: Vec<(&str, &[u8])> = files.iter().map(|(path, contents)| (path.as_str(), *contents)).collect();
        archive(&files)
    }

    #[test]
    fn a_download_is_hashed_as_it_arrives() {
        let mut out = Vec::new();
        let mut heard = 0;
        let sha = receive(&b"abc"[..], &mut out, |n| heard = n).unwrap();
        assert_eq!(sha, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!((out, heard), (b"abc".to_vec(), 3));
    }

    #[test]
    fn only_the_model_with_its_hash_is_unpacked() {
        let root = std::env::temp_dir().join(format!("voice-model-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("download.tar.bz2");
        fs::write(&file, model_archive(&[("other/evil.txt", b"no"), ("evil.txt", b"no"), (&format!("{}/README.md", MODEL.id), b"no")])).unwrap();

        assert!(matches!(unpack(&file, "0000", &root), Err(VoiceError::Damaged)));
        assert!(!installed(&root));

        unpack(&file, MODEL.sha256, &root).unwrap();
        assert!(installed(&root));
        assert!(!model_dir(&root).join("README.md").exists());
        assert!(!root.join("evil.txt").exists() && !root.join("other").exists() && !model_dir(&root).join("evil.txt").exists());

        remove(&root).unwrap();
        assert!(!installed(&root));
        remove(&root).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_archive_missing_a_file_installs_nothing() {
        let root = std::env::temp_dir().join(format!("voice-model-part-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let file = root.join("download.tar.bz2");
        fs::write(&file, archive(&[(&format!("{}/tts.json", MODEL.id), b"{}")])).unwrap();
        assert!(matches!(unpack(&file, MODEL.sha256, &root), Err(VoiceError::Damaged)));
        assert!(!model_dir(&root).exists() && !installed(&root));
        let _ = fs::remove_dir_all(&root);
    }
}
