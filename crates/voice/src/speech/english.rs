//! English numbers in words, as said in Britain (the app's English voice
//! is en-GB): cardinals with "and", ordinals, years, decimals and times.

const UNITS: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen",
    "seventeen", "eighteen", "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
const SCALES: [(u64, &str); 3] = [(1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand")];

fn below_hundred(n: u64) -> String {
    let n = n as usize;
    match n {
        0..20 => UNITS[n].into(),
        _ if n.is_multiple_of(10) => TENS[n / 10].into(),
        _ => format!("{}-{}", TENS[n / 10], UNITS[n % 10]),
    }
}

fn below_thousand(n: u64) -> String {
    let (hundreds, rest) = (n / 100, n % 100);
    match (hundreds, rest) {
        (0, _) => below_hundred(rest),
        (_, 0) => format!("{} hundred", UNITS[hundreds as usize]),
        _ => format!("{} hundred and {}", UNITS[hundreds as usize], below_hundred(rest)),
    }
}

/// `n` in words: "one thousand two hundred and fifty".
pub(crate) fn cardinal(n: u64) -> String {
    if n == 0 {
        return UNITS[0].into();
    }
    let mut words = Vec::new();
    let mut rest = n;
    for (size, name) in SCALES {
        if rest >= size {
            words.push(format!("{} {name}", cardinal(rest / size)));
            rest %= size;
        }
    }
    match rest {
        0 => {}
        1..100 if !words.is_empty() => words.push(format!("and {}", below_hundred(rest))),
        _ => words.push(below_thousand(rest)),
    }
    words.join(" ")
}

/// `n`th in words: "twenty-first".
pub(crate) fn ordinal(n: u64) -> String {
    let words = cardinal(n);
    let split = words.rfind([' ', '-']).map_or(0, |at| at + 1);
    let (head, last) = words.split_at(split);
    let last = match last {
        "one" => "first".to_string(),
        "two" => "second".into(),
        "three" => "third".into(),
        "five" => "fifth".into(),
        "eight" => "eighth".into(),
        "nine" => "ninth".into(),
        "twelve" => "twelfth".into(),
        _ => match last.strip_suffix('y') {
            Some(stem) => format!("{stem}ieth"),
            None => format!("{last}th"),
        },
    };
    format!("{head}{last}")
}

/// `n` as a year is said, "nineteen forty-five"; none for a number not
/// said that way (2000 to 2009 are "two thousand and five").
pub(crate) fn year(n: u64) -> Option<String> {
    if !(1100..2000).contains(&n) && !(2010..2100).contains(&n) {
        return None;
    }
    let (high, low) = (n / 100, n % 100);
    Some(match low {
        0 => format!("{} hundred", below_hundred(high)),
        1..10 => format!("{} oh {}", below_hundred(high), UNITS[low as usize]),
        _ => format!("{} {}", below_hundred(high), below_hundred(low)),
    })
}

/// A decimal number: "three point one four".
pub(crate) fn decimal(whole: u64, fraction: &str) -> String {
    format!("{} point {}", cardinal(whole), digits(fraction))
}

/// Each digit on its own: "zero zero seven".
pub(crate) fn digits(digits: &str) -> String {
    digits.chars().filter_map(|c| c.to_digit(10)).map(|d| UNITS[d as usize]).collect::<Vec<_>>().join(" ")
}

/// A time of day: "seven o'clock", "eight oh five", "twelve thirty".
pub(crate) fn time(hours: u64, minutes: u64) -> String {
    match minutes {
        0 => format!("{} o'clock", cardinal(hours)),
        1..10 => format!("{} oh {}", cardinal(hours), UNITS[minutes as usize]),
        _ => format!("{} {}", cardinal(hours), below_hundred(minutes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinals_and_ordinals() {
        assert_eq!(cardinal(105), "one hundred and five");
        assert_eq!(cardinal(21_000), "twenty-one thousand");
        assert_eq!(cardinal(1_005), "one thousand and five");
        assert_eq!(cardinal(3_000_250), "three million two hundred and fifty");
        assert_eq!((ordinal(1), ordinal(12), ordinal(20), ordinal(103)), ("first".into(), "twelfth".into(), "twentieth".into(), "one hundred and third".into()));
    }

    #[test]
    fn years() {
        assert_eq!(year(1900).unwrap(), "nineteen hundred");
        assert_eq!(year(1905).unwrap(), "nineteen oh five");
        assert_eq!(year(2024).unwrap(), "twenty twenty-four");
        assert_eq!(year(2005), None);
        assert_eq!(year(500), None);
    }
}
