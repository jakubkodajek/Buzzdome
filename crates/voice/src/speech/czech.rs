//! Czech numbers in words: cardinals that agree with a gender, ordinals
//! in a gender or the genitive, decimals, digits and times.

use super::{Form, Gender};

const UNITS: [&str; 20] = [
    "nula", "jedna", "dva", "tři", "čtyři", "pět", "šest", "sedm", "osm", "devět", "deset", "jedenáct", "dvanáct", "třináct", "čtrnáct", "patnáct",
    "šestnáct", "sedmnáct", "osmnáct", "devatenáct",
];
const TENS: [&str; 10] = ["", "", "dvacet", "třicet", "čtyřicet", "padesát", "šedesát", "sedmdesát", "osmdesát", "devadesát"];
const HUNDREDS: [&str; 10] = ["", "sto", "dvě stě", "tři sta", "čtyři sta", "pět set", "šest set", "sedm set", "osm set", "devět set"];

/// Thousands, millions and billions: the size, its gender, and its word
/// for one, two to four and more.
const SCALES: [(u64, Gender, [&str; 3]); 3] = [
    (1_000_000_000, Gender::Feminine, ["miliarda", "miliardy", "miliard"]),
    (1_000_000, Gender::Masculine, ["milion", "miliony", "milionů"]),
    (1_000, Gender::Masculine, ["tisíc", "tisíce", "tisíc"]),
];

const ORDINAL_UNITS: [&str; 20] = [
    "", "první", "druhý", "třetí", "čtvrtý", "pátý", "šestý", "sedmý", "osmý", "devátý", "desátý", "jedenáctý", "dvanáctý", "třináctý", "čtrnáctý",
    "patnáctý", "šestnáctý", "sedmnáctý", "osmnáctý", "devatenáctý",
];
const ORDINAL_TENS: [&str; 10] = ["", "", "dvacátý", "třicátý", "čtyřicátý", "padesátý", "šedesátý", "sedmdesátý", "osmdesátý", "devadesátý"];
const ORDINAL_HUNDREDS: [&str; 10] = ["", "stý", "dvoustý", "třístý", "čtyřstý", "pětistý", "šestistý", "sedmistý", "osmistý", "devítistý"];

/// `n` in words. Its gender is that of what it counts, which only one and
/// two show ("jeden kilometr", "dvě koruny"); none counts ("jedna, dva").
pub(crate) fn cardinal(n: u64, gender: Option<Gender>) -> String {
    match (n, gender) {
        (0, _) => UNITS[0].into(),
        (1, Some(Gender::Masculine)) => "jeden".into(),
        (1, Some(Gender::Neuter)) => "jedno".into(),
        (2, Some(Gender::Feminine | Gender::Neuter)) => "dvě".into(),
        _ => {
            let mut words = Vec::new();
            let mut rest = n;
            for (size, scale_gender, names) in SCALES {
                let count = rest / size;
                rest %= size;
                match count {
                    0 => {}
                    1 => words.push(names[0].to_string()),
                    2..=4 => words.push(format!("{} {}", cardinal(count, Some(scale_gender)), names[1])),
                    _ => words.push(format!("{} {}", cardinal(count, None), names[2])),
                }
            }
            if rest > 0 {
                words.push(below_thousand(rest));
            }
            words.join(" ")
        }
    }
}

fn below_thousand(n: u64) -> String {
    let (hundreds, rest) = ((n / 100) as usize, (n % 100) as usize);
    let mut words = Vec::new();
    if hundreds > 0 {
        words.push(HUNDREDS[hundreds]);
    }
    if rest >= 20 {
        words.push(TENS[rest / 10]);
        if rest % 10 > 0 {
            words.push(UNITS[rest % 10]);
        }
    } else if rest > 0 {
        words.push(UNITS[rest]);
    }
    words.join(" ")
}

/// The ending of an adjective such as "takzvaný" in `gender`.
pub(crate) fn ending(gender: Gender) -> &'static str {
    match gender {
        Gender::Masculine => "ý",
        Gender::Feminine => "á",
        Gender::Neuter => "é",
    }
}

/// `n`th in words, 1 to 999, in `form`; none for others, which are read
/// as numbers.
pub(crate) fn ordinal(n: u64, form: Form) -> Option<String> {
    if !(1..1000).contains(&n) {
        return None;
    }
    let (hundreds, rest) = ((n / 100) as usize, (n % 100) as usize);
    let mut words = Vec::new();
    if hundreds > 0 {
        words.push(ORDINAL_HUNDREDS[hundreds]);
    }
    if rest >= 20 {
        words.push(ORDINAL_TENS[rest / 10]);
        if rest % 10 > 0 {
            words.push(ORDINAL_UNITS[rest % 10]);
        }
    } else if rest > 0 {
        words.push(ORDINAL_UNITS[rest]);
    }
    Some(words.into_iter().map(|word| inflected(word, form)).collect::<Vec<_>>().join(" "))
}

/// An ordinal's word, written masculine, in `form`: "druhý" is "druhá",
/// "druhé", "druhého" or "druhém"; "třetí" stays but for "třetího" and
/// "třetím".
fn inflected(word: &str, form: Form) -> String {
    let (stem, soft) = match (word.strip_suffix('ý'), word.strip_suffix('í')) {
        (Some(stem), _) => (stem, false),
        (None, Some(stem)) => (stem, true),
        _ => return word.into(),
    };
    let end = match (form, soft) {
        (Form::Nominative(gender), false) => ending(gender),
        (Form::Nominative(_) | Form::Genitive(Gender::Feminine) | Form::Locative(Gender::Feminine), true) => "í",
        (Form::Genitive(Gender::Feminine) | Form::Locative(Gender::Feminine), false) => "é",
        (Form::Genitive(_), false) => "ého",
        (Form::Genitive(_), true) => "ího",
        (Form::Locative(_), false) => "ém",
        (Form::Locative(_), true) => "ím",
    };
    format!("{stem}{end}")
}

/// A decimal number: "tři celé čtrnáct". A part after the comma that
/// starts with a nought is read digit by digit.
pub(crate) fn decimal(whole: u64, fraction: &str) -> String {
    let point = match whole {
        1 => "celá",
        2..=4 => "celé",
        _ => "celých",
    };
    let after = match fraction.parse::<u64>() {
        Ok(n) if !fraction.starts_with('0') && fraction.len() <= 3 => cardinal(n, None),
        _ => digits(fraction),
    };
    format!("{} {point} {after}", cardinal(whole, Some(Gender::Feminine)))
}

/// Each digit on its own: "nula nula sedm".
pub(crate) fn digits(digits: &str) -> String {
    digits.chars().filter_map(|c| c.to_digit(10)).map(|d| UNITS[d as usize]).collect::<Vec<_>>().join(" ")
}

/// A time of day: "dvanáct třicet", "osm nula pět".
pub(crate) fn time(hours: u64, minutes: u64) -> String {
    let minutes = match minutes {
        0 => "nula nula".to_string(),
        1..=9 => format!("nula {}", UNITS[minutes as usize]),
        _ => cardinal(minutes, None),
    };
    format!("{} {minutes}", cardinal(hours, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinals() {
        let cases = [
            (0, "nula"),
            (13, "třináct"),
            (21, "dvacet jedna"),
            (100, "sto"),
            (200, "dvě stě"),
            (345, "tři sta čtyřicet pět"),
            (1000, "tisíc"),
            (2000, "dva tisíce"),
            (5000, "pět tisíc"),
            (1348, "tisíc tři sta čtyřicet osm"),
            (1_000_000, "milion"),
            (3_500_000, "tři miliony pět set tisíc"),
            (2_000_000_000, "dvě miliardy"),
            (7_000_000_000, "sedm miliard"),
        ];
        for (n, words) in cases {
            assert_eq!(cardinal(n, None), words, "{n}");
        }
        assert_eq!((cardinal(1, Some(Gender::Masculine)), cardinal(2, Some(Gender::Feminine)), cardinal(2, None)), ("jeden".into(), "dvě".into(), "dva".into()));
    }

    #[test]
    fn ordinals() {
        assert_eq!(ordinal(1, Form::Nominative(Gender::Feminine)).unwrap(), "první");
        assert_eq!(ordinal(2, Form::Nominative(Gender::Feminine)).unwrap(), "druhá");
        assert_eq!(ordinal(14, Form::Nominative(Gender::Neuter)).unwrap(), "čtrnácté");
        assert_eq!(ordinal(17, Form::DATE).unwrap(), "sedmnáctého");
        assert_eq!(ordinal(3, Form::DATE).unwrap(), "třetího");
        assert_eq!(ordinal(2, Form::Genitive(Gender::Feminine)).unwrap(), "druhé");
        assert_eq!(ordinal(14, Form::Locative(Gender::Neuter)).unwrap(), "čtrnáctém");
        assert_eq!(ordinal(3, Form::Locative(Gender::Masculine)).unwrap(), "třetím");
        assert_eq!(ordinal(3, Form::Locative(Gender::Feminine)).unwrap(), "třetí");
        assert_eq!(ordinal(21, Form::Nominative(Gender::Masculine)).unwrap(), "dvacátý první");
        assert_eq!(ordinal(100, Form::Nominative(Gender::Masculine)).unwrap(), "stý");
        assert_eq!(ordinal(1000, Form::DATE), None);
    }

    #[test]
    fn decimals_and_times() {
        assert_eq!(decimal(1, "5"), "jedna celá pět");
        assert_eq!(decimal(2, "25"), "dvě celé dvacet pět");
        assert_eq!(decimal(10, "05"), "deset celých nula pět");
        assert_eq!(time(0, 0), "nula nula nula");
    }
}
