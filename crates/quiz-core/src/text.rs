//! How names are kept and compared: the same letters are the same name,
//! whatever their case and however their accents are encoded. A name typed
//! on one system and pasted from another can encode "é" as one character
//! or as "e" and an accent (macOS file names do), which look the same.

use unicode_normalization::UnicodeNormalization;

/// `text` with its accents encoded as single characters (NFC), the form
/// names are kept in.
pub(crate) fn composed(text: &str) -> String {
    text.nfc().collect()
}

/// The key names are told apart by: without the spaces around them,
/// composed and in lower case.
pub(crate) fn name_key(name: &str) -> String {
    name.trim().to_lowercase().nfc().collect()
}

/// The key file names are told apart by on some disk the quiz may be
/// opened on. Windows disks compare names in upper case, a letter for a
/// letter; macOS disks compare their caseless form, which folds case fully
/// (Unicode's canonical caseless match, D145, per `bsd/sys/unicode.h` in
/// Apple's xnu), so `ß` matches `ss` there. The key is the name in upper
/// case, then in lower case (`σ` and `ς` are one letter in upper case, and
/// `ß` becomes `ss`), and composed, as macOS disks also ignore how accents
/// are encoded. Two media files whose names have one key are one file on
/// Windows or on macOS.
pub fn file_key(name: &str) -> String {
    name.to_uppercase().to_lowercase().nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_differing_in_case_or_in_how_accents_are_encoded_have_one_key() {
        let decomposed = "Z\u{30c}lut\u{30c}ouc\u{30c}ky\u{301} ku\u{30a}n\u{30c}";
        assert_eq!(name_key(decomposed), name_key(" ŽLUŤOUČKÝ kůň "));
        assert_eq!(composed(decomposed), "Žluťoučký kůň");
        assert_ne!(name_key("kun"), name_key("kůň"), "accents still count");
        assert_eq!(file_key("media/Ku\u{30a}n\u{30c}.PNG"), file_key("media/kůň.png"));
        // Windows folds names to upper case, where the two sigmas are one letter.
        assert_eq!(file_key("media/ς.png"), file_key("media/σ.png"));
        // Two files on Windows, one on macOS.
        assert_eq!(file_key("media/Straße.png"), file_key("media/STRASSE.png"));
        assert_ne!(file_key("media/ kůň.png"), file_key("media/kůň.png"), "spaces are part of a file name");
    }
}
