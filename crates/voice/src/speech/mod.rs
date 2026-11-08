//! Writes a question's text the way it is said, before the voice model
//! reads it: numbers, dates, times, ordinals, Roman numerals, units and
//! abbreviations become words of the quiz's language. The model reads
//! digits and symbols on its own only in part (it said "1945" as "de roce
//! 45" and "-5 °C" as "pět doce" in our test, `docs/design.md` 1.8), so
//! the app writes them out. What these rules get wrong, a question's
//! "read as" text says instead (`quiz_core::Question::read_as`).
//!
//! Czech has cases and genders that rules alone cannot always tell: a
//! number takes the gender of the unit after it, an ordinal the ending of
//! the word after it, and everything is in the nominative but for the day
//! and month of a date.

mod czech;
mod english;

use quiz_core::QuizLanguage;

/// The grammatical gender a Czech number or ordinal agrees with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gender {
    Masculine,
    Feminine,
    Neuter,
}

/// The case and gender of a Czech ordinal: "druhá světová válka", "do
/// druhé světové války", "po druhé světové válce"; a date's day and
/// month are in the masculine genitive ("prvního ledna").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    Nominative(Gender),
    Genitive(Gender),
    Locative(Gender),
}

impl Form {
    const DATE: Form = Form::Genitive(Gender::Masculine);
}

/// Czech prepositions that the words after them follow in the locative,
/// and in the genitive (the common ones only).
const CZECH_LOCATIVE: &[&str] = &["v", "ve", "na", "o", "po", "při"];
const CZECH_GENITIVE: &[&str] = &["do", "od", "ode", "z", "ze", "u", "během", "kolem", "podle", "bez", "místo", "okolo", "vedle", "uprostřed"];

/// How many of a unit there are, which picks its word: one, two to four
/// (Czech has a form of its own for them), more, or a decimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Count {
    One,
    Few,
    Many,
    Fraction,
}

impl Count {
    fn of(n: u64) -> Count {
        match n {
            1 => Count::One,
            2..=4 => Count::Few,
            _ => Count::Many,
        }
    }
}

/// A unit or currency written after a number (or a currency before it):
/// its Czech gender and words by `Count`, and its English words.
struct Unit {
    symbol: &'static str,
    gender: Gender,
    czech: [&'static str; 4],
    english: [&'static str; 2],
    /// Written before the number too ("$5").
    prefix: bool,
}

const fn unit(symbol: &'static str, gender: Gender, czech: [&'static str; 4], english: [&'static str; 2]) -> Unit {
    Unit { symbol, gender, czech, english, prefix: false }
}

const fn currency(symbol: &'static str, gender: Gender, czech: [&'static str; 4], english: [&'static str; 2]) -> Unit {
    Unit { symbol, gender, czech, english, prefix: true }
}

use Gender::{Feminine as F, Masculine as M, Neuter as N};

/// The units, a longer symbol before one it starts with.
const UNITS: &[Unit] = &[
    unit("km/h", M, ["kilometr za hodinu", "kilometry za hodinu", "kilometrů za hodinu", "kilometru za hodinu"], ["kilometre per hour", "kilometres per hour"]),
    unit("km²", M, ["čtvereční kilometr", "čtvereční kilometry", "čtverečních kilometrů", "čtverečního kilometru"], ["square kilometre", "square kilometres"]),
    unit("m²", M, ["čtvereční metr", "čtvereční metry", "čtverečních metrů", "čtverečního metru"], ["square metre", "square metres"]),
    unit("°C", M, ["stupeň Celsia", "stupně Celsia", "stupňů Celsia", "stupně Celsia"], ["degree Celsius", "degrees Celsius"]),
    unit("°F", M, ["stupeň Fahrenheita", "stupně Fahrenheita", "stupňů Fahrenheita", "stupně Fahrenheita"], ["degree Fahrenheit", "degrees Fahrenheit"]),
    unit("°", M, ["stupeň", "stupně", "stupňů", "stupně"], ["degree", "degrees"]),
    unit("%", N, ["procento", "procenta", "procent", "procenta"], ["percent", "percent"]),
    unit("mph", F, ["míle za hodinu", "míle za hodinu", "mil za hodinu", "míle za hodinu"], ["mile per hour", "miles per hour"]),
    unit("km", M, ["kilometr", "kilometry", "kilometrů", "kilometru"], ["kilometre", "kilometres"]),
    unit("cm", M, ["centimetr", "centimetry", "centimetrů", "centimetru"], ["centimetre", "centimetres"]),
    unit("mm", M, ["milimetr", "milimetry", "milimetrů", "milimetru"], ["millimetre", "millimetres"]),
    unit("min", F, ["minuta", "minuty", "minut", "minuty"], ["minute", "minutes"]),
    unit("mg", M, ["miligram", "miligramy", "miligramů", "miligramu"], ["milligram", "milligrams"]),
    unit("ml", M, ["mililitr", "mililitry", "mililitrů", "mililitru"], ["millilitre", "millilitres"]),
    unit("kg", M, ["kilogram", "kilogramy", "kilogramů", "kilogramu"], ["kilogram", "kilograms"]),
    unit("m", M, ["metr", "metry", "metrů", "metru"], ["metre", "metres"]),
    unit("g", M, ["gram", "gramy", "gramů", "gramu"], ["gram", "grams"]),
    unit("l", M, ["litr", "litry", "litrů", "litru"], ["litre", "litres"]),
    unit("t", F, ["tuna", "tuny", "tun", "tuny"], ["tonne", "tonnes"]),
    unit("h", F, ["hodina", "hodiny", "hodin", "hodiny"], ["hour", "hours"]),
    unit("tis.", M, ["tisíc", "tisíce", "tisíc", "tisíce"], ["thousand", "thousand"]),
    unit("mil.", M, ["milion", "miliony", "milionů", "milionu"], ["million", "million"]),
    unit("mld.", F, ["miliarda", "miliardy", "miliard", "miliardy"], ["billion", "billion"]),
    unit("Kč", F, ["koruna", "koruny", "korun", "koruny"], ["Czech crown", "Czech crowns"]),
    currency("€", N, ["euro", "eura", "eur", "eura"], ["euro", "euros"]),
    currency("$", M, ["dolar", "dolary", "dolarů", "dolaru"], ["dollar", "dollars"]),
    currency("£", F, ["libra", "libry", "liber", "libry"], ["pound", "pounds"]),
];

/// An abbreviation and what it stands for.
struct Abbreviation {
    short: &'static str,
    long: &'static str,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Its dot also ends a sentence when the next one starts after it.
    Plain,
    /// A stem that takes the ending of the word after it ("tzv. válka" is
    /// "takzvaná válka").
    Gendered,
    /// "sv.": a stem before a saint's name, which after another word is in
    /// the genitive ("katedrála sv. Víta" is "svatého Víta").
    Saint,
    /// A title before a name, never the end of a sentence.
    Title,
}

const fn abbr(short: &'static str, long: &'static str) -> Abbreviation {
    Abbreviation { short, long, kind: Kind::Plain }
}

const fn title(short: &'static str, long: &'static str) -> Abbreviation {
    Abbreviation { short, long, kind: Kind::Title }
}

const CZECH_ABBREVIATIONS: &[Abbreviation] = &[
    abbr("př. n. l.", "před naším letopočtem"),
    abbr("př.n.l.", "před naším letopočtem"),
    abbr("n. l.", "našeho letopočtu"),
    abbr("n.l.", "našeho letopočtu"),
    abbr("např.", "například"),
    Abbreviation { short: "tzv.", long: "takzvan", kind: Kind::Gendered },
    Abbreviation { short: "sv.", long: "svat", kind: Kind::Saint },
    abbr("atd.", "a tak dále"),
    abbr("apod.", "a podobně"),
    abbr("tj.", "to jest"),
    abbr("tzn.", "to znamená"),
    abbr("mj.", "mimo jiné"),
    abbr("resp.", "respektive"),
    abbr("popř.", "popřípadě"),
    abbr("cca", "cirka"),
    abbr("č.", "číslo"),
    abbr("str.", "strana"),
];

const ENGLISH_ABBREVIATIONS: &[Abbreviation] = &[
    abbr("e.g.", "for example"),
    abbr("i.e.", "that is"),
    abbr("etc.", "et cetera"),
    abbr("vs.", "versus"),
    abbr("approx.", "approximately"),
    title("Mr.", "Mister"),
    title("Mrs.", "Missus"),
    title("Dr.", "Doctor"),
];

/// The Czech month names in the genitive, as a date's month is written.
const CZECH_MONTHS: [&str; 12] = ["ledna", "února", "března", "dubna", "května", "června", "července", "srpna", "září", "října", "listopadu", "prosince"];

/// English words after which a Roman numeral is a number ("World War II"),
/// not a ruler's ordinal ("Henry VIII").
const ENGLISH_COUNTED: &[&str] = &["War", "Part", "Chapter", "Act", "Volume", "Book", "Episode", "Phase", "Bowl", "Type", "Class", "Stage", "Level", "Grade", "Article", "Apollo"];

/// The longest number read as one (below a million million); longer ones
/// are read digit by digit, as a phone number is.
const MAX_DIGITS: usize = 12;

/// `text` written as it is said in `language`.
pub fn spoken(text: &str, language: QuizLanguage) -> String {
    let expanded = expand_abbreviations(text, language);
    Reader { chars: expanded.chars().collect(), language, out: String::new() }.read()
}

/// The letter of answer `index` (from 0) as it is said, alone: Czech
/// letters are written as they sound, so that "A" is not read as "a".
pub fn letter(index: usize, language: QuizLanguage) -> &'static str {
    const CZECH: [&str; 6] = ["Á", "Bé", "Cé", "Dé", "É", "Ef"];
    const ENGLISH: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
    let letters = match language {
        QuizLanguage::Cs => &CZECH,
        QuizLanguage::En => &ENGLISH,
    };
    letters.get(index).copied().unwrap_or("")
}

/// The form of a Czech ordinal between `before` (a preposition sets the
/// case) and `after` (its ending tells the gender), by rules of thumb
/// that hold for most nouns and adjectives after them.
fn ordinal_form(before: Option<&str>, after: &str) -> Form {
    let preposition = before.map(str::to_lowercase);
    let is = |list: &[&str]| preposition.as_deref().is_some_and(|word| list.contains(&word));
    let ends = |endings: &[&str]| endings.iter().any(|end| after.ends_with(end));
    if is(CZECH_LOCATIVE) {
        // "ve druhém kole", "ve čtrnáctém století", "po druhé světové válce"
        return Form::Locative(if ends(&["ém", "u", "í"]) || !ends(&["é", "e", "ě", "i"]) { Gender::Masculine } else { Gender::Feminine });
    }
    if is(CZECH_GENITIVE) {
        // "do druhého kola", "od devatenáctého století", "do druhé světové války"
        return Form::Genitive(if ends(&["é", "y", "e", "ě", "i"]) && !ends(&["ého"]) { Gender::Feminine } else { Gender::Masculine });
    }
    Form::Nominative(gender_before(after))
}

/// The gender a Czech ordinal before `word` takes, by how the word ends:
/// a rule of thumb that holds for most nouns and adjectives.
fn gender_before(word: &str) -> Gender {
    match word.chars().last().map(|c| c.to_lowercase().next().unwrap_or(c)) {
        Some('a' | 'á') => Gender::Feminine,
        Some('o' | 'é' | 'í' | 'e' | 'ě') => Gender::Neuter,
        _ => Gender::Masculine,
    }
}

fn is_space(c: char) -> bool {
    c.is_whitespace()
}

/// Whether the text after `rest` starts a new sentence: it ends there,
/// or a capital letter comes after the spaces.
fn sentence_ends(rest: &[char]) -> bool {
    match rest.iter().position(|c| !is_space(*c)) {
        None => true,
        Some(at) => at > 0 && rest[at].is_uppercase(),
    }
}

/// The word that starts after the spaces at the start of `rest`, if any.
fn next_word(rest: &[char]) -> Option<String> {
    let start = rest.iter().position(|c| !is_space(*c))?;
    if start == 0 {
        return None;
    }
    let word: String = rest[start..].iter().take_while(|c| c.is_alphabetic()).collect();
    (!word.is_empty()).then_some(word)
}

fn expand_abbreviations(text: &str, language: QuizLanguage) -> String {
    let table = match language {
        QuizLanguage::Cs => CZECH_ABBREVIATIONS,
        QuizLanguage::En => ENGLISH_ABBREVIATIONS,
    };
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'scan: while i < chars.len() {
        if i == 0 || !chars[i - 1].is_alphanumeric() {
            for abbreviation in table {
                let Some(end) = matches_at(&chars, i, abbreviation.short) else { continue };
                if chars.get(end).is_some_and(|c| c.is_alphanumeric()) {
                    continue;
                }
                let rest = &chars[end..];
                let mut long = abbreviation.long.to_string();
                let word_after = next_word(rest).unwrap_or_default();
                let after_a_word = out.trim_end().ends_with(char::is_alphabetic);
                match abbreviation.kind {
                    Kind::Gendered => long.push_str(czech::ending(gender_before(&word_after))),
                    Kind::Saint if after_a_word => long.push_str(if word_after.ends_with(['y', 'i']) { "é" } else { "ého" }),
                    Kind::Saint => long.push_str(czech::ending(gender_before(&word_after))),
                    Kind::Plain | Kind::Title => {}
                }
                if chars[i].is_uppercase() && !abbreviation.short.starts_with(char::is_uppercase) {
                    long = capitalized(&long);
                }
                out.push_str(&long);
                if abbreviation.kind == Kind::Plain && abbreviation.short.ends_with('.') && sentence_ends(rest) {
                    out.push('.');
                }
                i = end;
                continue 'scan;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Where `short` ends when it is written at `at`, its first letter in
/// either case.
fn matches_at(chars: &[char], at: usize, short: &str) -> Option<usize> {
    let mut i = at;
    for (n, expected) in short.chars().enumerate() {
        let found = *chars.get(i)?;
        let same = if n == 0 { found.to_lowercase().eq(expected.to_lowercase()) } else { found == expected };
        if !same {
            return None;
        }
        i += 1;
    }
    Some(i)
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// A number as written: whole, with a decimal part, or a run of digits
/// that is read one by one.
#[derive(Debug, Clone, PartialEq)]
enum Number {
    Whole(u64),
    Decimal(u64, String),
    Digits(String),
}

impl Number {
    fn count(&self) -> Count {
        match self {
            Number::Whole(n) => Count::of(*n),
            Number::Decimal(..) => Count::Fraction,
            Number::Digits(_) => Count::Many,
        }
    }
}

struct Reader {
    chars: Vec<char>,
    language: QuizLanguage,
    out: String,
}

impl Reader {
    fn read(mut self) -> String {
        let mut i = 0;
        while i < self.chars.len() {
            i = self.step(i);
        }
        self.out
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn starts_word(&self, i: usize) -> bool {
        i == 0 || !self.chars[i - 1].is_alphanumeric()
    }

    fn is_czech(&self) -> bool {
        self.language == QuizLanguage::Cs
    }

    /// Reads what starts at `i`, and says where the next thing starts.
    fn step(&mut self, i: usize) -> usize {
        let c = self.chars[i];
        let digit_after = self.at(i + 1).is_some_and(|c| c.is_ascii_digit());
        if c.is_ascii_digit() && self.starts_word(i) {
            return self.number(i, None);
        }
        if matches!(c, '-' | '−') && digit_after && (i == 0 || matches!(self.chars[i - 1], ' ' | '(' | '/' | '\u{a0}')) {
            self.out.push_str("minus ");
            return i + 1;
        }
        if let Some(currency) = UNITS.iter().find(|unit| unit.prefix && unit.symbol.starts_with(c)) {
            let digit = if self.at(i + 1) == Some(' ') { i + 2 } else { i + 1 };
            if self.at(digit).is_some_and(|c| c.is_ascii_digit()) {
                return self.number(digit, Some(currency));
            }
        }
        if c.is_ascii_uppercase() && self.starts_word(i) {
            if let Some(next) = self.roman(i) {
                return next;
            }
        }
        let between = |at: Option<char>| at.is_none_or(|c| c.is_ascii_digit() || c == ' ');
        let operator = match c {
            '+' => Some(["plus", "plus"]),
            '=' => Some(["rovná se", "equals"]),
            '×' => Some(["krát", "times"]),
            '&' => Some(["a", "and"]),
            _ => None,
        };
        if let Some(words) = operator.filter(|_| i > 0 && between(self.at(i - 1)) && between(self.at(i + 1))) {
            let word = words[usize::from(!self.is_czech())];
            let before = if self.out.ends_with(' ') { "" } else { " " };
            let after = if self.at(i + 1) == Some(' ') { "" } else { " " };
            self.out.push_str(&format!("{before}{word}{after}"));
            return i + 1;
        }
        self.out.push(c);
        i + 1
    }

    /// `at`, or past the one space there.
    fn past_space(&self, at: usize) -> usize {
        if self.at(at) == Some(' ') { at + 1 } else { at }
    }

    /// The digits from `i`, and where they end.
    fn digits(&self, i: usize) -> (String, usize) {
        let digits: String = self.chars[i..].iter().take_while(|c| c.is_ascii_digit()).collect();
        let end = i + digits.len();
        (digits, end)
    }

    /// The number at `i` as written, with its thousands grouped by spaces
    /// (Czech) or commas (English), and where it ends.
    fn parse_number(&self, i: usize) -> (Number, usize) {
        let (mut whole, mut end) = self.digits(i);
        let group = if self.is_czech() { [' ', '\u{a0}', '\u{202f}', '\u{2009}'].as_slice() } else { [','].as_slice() };
        if whole.len() <= 3 {
            while self.at(end).is_some_and(|c| group.contains(&c)) {
                let (next, next_end) = self.digits(end + 1);
                if next.len() != 3 {
                    break;
                }
                whole.push_str(&next);
                end = next_end;
            }
        }
        let point = if self.is_czech() { ',' } else { '.' };
        if self.at(end) == Some(point) && self.at(end + 1).is_some_and(|c| c.is_ascii_digit()) {
            let (fraction, fraction_end) = self.digits(end + 1);
            if let Ok(n) = whole.parse() {
                return (Number::Decimal(n, fraction), fraction_end);
            }
        }
        if whole.len() > MAX_DIGITS || (whole.len() > 1 && whole.starts_with('0')) {
            return (Number::Digits(whole), end);
        }
        (whole.parse().map_or(Number::Digits(whole), Number::Whole), end)
    }

    fn cardinal(&self, number: &Number, gender: Option<Gender>) -> String {
        match (number, self.language) {
            (Number::Whole(n), QuizLanguage::Cs) => czech::cardinal(*n, gender),
            (Number::Whole(n), QuizLanguage::En) => english::cardinal(*n),
            (Number::Decimal(n, fraction), QuizLanguage::Cs) => czech::decimal(*n, fraction),
            (Number::Decimal(n, fraction), QuizLanguage::En) => english::decimal(*n, fraction),
            (Number::Digits(digits), QuizLanguage::Cs) => czech::digits(digits),
            (Number::Digits(digits), QuizLanguage::En) => english::digits(digits),
        }
    }

    fn unit_words(&self, unit: &Unit, count: Count) -> &'static str {
        match self.language {
            QuizLanguage::Cs => unit.czech[count as usize],
            QuizLanguage::En => unit.english[usize::from(count != Count::One)],
        }
    }

    /// The unit written at `i` after a number, one space before it allowed,
    /// and where it ends.
    fn unit_at(&self, i: usize) -> Option<(&'static Unit, usize)> {
        let start = if matches!(self.at(i), Some(' ' | '\u{a0}' | '\u{202f}')) { i + 1 } else { i };
        UNITS.iter().find_map(|unit| {
            let end = start + unit.symbol.chars().count();
            let written = self.chars.get(start..end)?.iter().copied().eq(unit.symbol.chars());
            let alone = !unit.symbol.ends_with(char::is_alphabetic) || self.at(end).is_none_or(|c| !c.is_alphanumeric());
            (written && alone).then_some((unit, end))
        })
    }

    /// Reads the number at `i`, with what belongs to it: a date or time it
    /// is part of, an ordinal's ending, a unit after it, or `currency`
    /// written before it.
    fn number(&mut self, i: usize, currency: Option<&'static Unit>) -> usize {
        if currency.is_none() {
            if let Some(next) = self.date(i).or_else(|| self.time(i)) {
                return next;
            }
        }
        let (number, end) = self.parse_number(i);
        if currency.is_none() {
            if let Some(next) = self.ordinal(&number, i, end) {
                return next;
            }
        }
        let unit = currency.map(|currency| (currency, end)).or_else(|| self.unit_at(end));
        if let Some((unit, unit_end)) = unit {
            let words = self.cardinal(&number, Some(unit.gender));
            let named = self.unit_words(unit, number.count());
            self.out.push_str(&format!("{words} {named}"));
            // A unit of a unit ("5 mil. Kč") counts as many.
            let mut end = unit_end;
            if let Some((then, then_end)) = self.unit_at(end).filter(|_| unit.symbol.ends_with('.') || unit.symbol == "tis.") {
                self.out.push(' ');
                self.out.push_str(self.unit_words(then, Count::Many));
                end = then_end;
            }
            if unit.symbol.ends_with('.') && sentence_ends(&self.chars[end..]) {
                self.out.push('.');
            }
            return end;
        }
        let words = match (&number, self.language) {
            (Number::Whole(n), QuizLanguage::En) if end - i == 4 => english::year(*n).unwrap_or_else(|| english::cardinal(*n)),
            _ => self.cardinal(&number, None),
        };
        self.out.push_str(&words);
        self.range(end)
    }

    /// After a number ending at `end`: a dash to another number is a range,
    /// "1914–1918", read with "až" or "to". Where to go on.
    fn range(&mut self, end: usize) -> usize {
        let dash = self.past_space(end);
        let tight = self.at(end) == Some('-');
        let to = self.past_space(dash + 1);
        if (matches!(self.at(dash), Some('–' | '—')) || tight) && self.at(to).is_some_and(|c| c.is_ascii_digit()) {
            self.out.push_str(if self.is_czech() { " až " } else { " to " });
            return to;
        }
        end
    }

    /// A Czech date at `i`, "17. 11. 1989" or "17. 11.": the day and month
    /// as ordinals in the genitive, then the year. Where it ends.
    fn date(&mut self, i: usize) -> Option<usize> {
        if !self.is_czech() {
            return None;
        }
        let (day, day_end) = self.digits(i);
        if day.len() > 2 || self.at(day_end) != Some('.') {
            return None;
        }
        let (month, month_end) = self.digits(self.past_space(day_end + 1));
        if month.is_empty() || month.len() > 2 || self.at(month_end) != Some('.') {
            return None;
        }
        let (day, month): (u64, u64) = (day.parse().ok()?, month.parse().ok()?);
        if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
            return None;
        }
        self.out.push_str(&format!("{} {}", czech::ordinal(day, Form::DATE)?, czech::ordinal(month, Form::DATE)?));
        let after_month = month_end + 1;
        let (year, year_end) = self.digits(self.past_space(after_month));
        if year.len() == 4 && self.at(year_end).is_none_or(|c| !c.is_alphanumeric()) {
            self.out.push(' ');
            self.out.push_str(&czech::cardinal(year.parse().ok()?, None));
            return Some(year_end);
        }
        // The month's dot ends the sentence too ("Bylo to 17. 11.").
        if sentence_ends(&self.chars[after_month..]) {
            self.out.push('.');
        }
        Some(after_month)
    }

    /// A time at `i`, "12:30". Where it ends.
    fn time(&mut self, i: usize) -> Option<usize> {
        let (hours, hours_end) = self.digits(i);
        if hours.len() > 2 || self.at(hours_end) != Some(':') {
            return None;
        }
        let (minutes, minutes_end) = self.digits(hours_end + 1);
        if minutes.len() != 2 || self.at(minutes_end).is_some_and(|c| c.is_alphanumeric()) {
            return None;
        }
        let (hours, minutes): (u64, u64) = (hours.parse().ok()?, minutes.parse().ok()?);
        if hours > 23 || minutes > 59 {
            return None;
        }
        let words = match self.language {
            QuizLanguage::Cs => czech::time(hours, minutes),
            QuizLanguage::En => english::time(hours, minutes),
        };
        self.out.push_str(&words);
        Some(minutes_end)
    }

    /// An ordinal: a Czech number with a dot before a word in small letters
    /// ("2. světová válka", "17. listopadu"), or an English one with its
    /// ending ("21st"). Where it ends.
    fn ordinal(&mut self, number: &Number, start: usize, end: usize) -> Option<usize> {
        let Number::Whole(n) = *number else { return None };
        match self.language {
            QuizLanguage::Cs => {
                if self.at(end) != Some('.') {
                    return None;
                }
                let word = next_word(&self.chars[end + 1..]).filter(|word| word.starts_with(char::is_lowercase))?;
                let form = if CZECH_MONTHS.contains(&word.as_str()) { Form::DATE } else { ordinal_form(self.word_before(start).as_deref(), &word) };
                self.out.push_str(&czech::ordinal(n, form)?);
                Some(end + 1)
            }
            QuizLanguage::En => {
                let ending: String = self.chars.get(end..end + 2)?.iter().collect::<String>().to_lowercase();
                let fits = matches!((ending.as_str(), n % 10, n % 100), ("st", 1, _) | ("nd", 2, _) | ("rd", 3, _) | ("th", _, _));
                if !fits || self.at(end + 2).is_some_and(|c| c.is_alphanumeric()) {
                    return None;
                }
                self.out.push_str(&english::ordinal(n));
                Some(end + 2)
            }
        }
    }

    /// A Roman numeral at `i`: a ruler's ("Karel IV.", "Henry VIII"), a
    /// century's ("XX. století"), or a counted one ("World War II"). Where
    /// it ends; none when it is not one.
    fn roman(&mut self, i: usize) -> Option<usize> {
        let numeral: String = self.chars[i..].iter().take_while(|c| "IVXLCDM".contains(**c)).collect();
        let end = i + numeral.chars().count();
        if self.at(end).is_some_and(|c| c.is_alphanumeric()) {
            return None;
        }
        let n = roman_value(&numeral)?;
        let before = self.word_before(i);
        let after_name = before.as_deref().is_some_and(is_name);
        let single = numeral.len() == 1;
        match self.language {
            QuizLanguage::Cs => {
                let dotted = self.at(end) == Some('.');
                let rest = &self.chars[end + usize::from(dotted)..];
                let word_after = next_word(rest).filter(|word| word.starts_with(char::is_lowercase));
                // A ruler is followed by what they did ("Karel IV. založil")
                // or by nothing; a century by its noun ("XX. století").
                let ruler = after_name && (!single || dotted) && word_after.as_deref().is_none_or(is_verb);
                let form = match word_after {
                    _ if ruler => Form::Nominative(regnal_gender(before.as_deref().unwrap_or_default())),
                    Some(word) if dotted => ordinal_form(before.as_deref(), &word),
                    _ => return None,
                };
                self.out.push_str(&czech::ordinal(n, form)?);
                if dotted && sentence_ends(rest) {
                    self.out.push('.');
                }
                Some(end + usize::from(dotted))
            }
            QuizLanguage::En => {
                if single || !after_name && before.is_none() {
                    return None;
                }
                let counted = before.as_deref().is_some_and(|word| ENGLISH_COUNTED.contains(&word));
                if counted {
                    self.out.push_str(&english::cardinal(n));
                } else if after_name {
                    self.out.push_str("the ");
                    self.out.push_str(&english::ordinal(n));
                } else {
                    return None;
                }
                Some(end)
            }
        }
    }

    /// The word just before `i`, with only spaces between.
    fn word_before(&self, i: usize) -> Option<String> {
        let mut end = i;
        while end > 0 && self.chars[end - 1] == ' ' {
            end -= 1;
        }
        if end == i {
            return None;
        }
        let start = self.chars[..end].iter().rposition(|c| !c.is_alphabetic()).map_or(0, |at| at + 1);
        (start < end).then(|| self.chars[start..end].iter().collect())
    }
}

/// Whether `word` looks like a name: a capital letter, then at least two
/// small ones (so that "Ve" and "Na" starting a sentence are not).
fn is_name(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(char::is_uppercase) && word.chars().count() > 2 && chars.all(char::is_lowercase)
}

/// Whether a Czech `word` after a ruler's numeral says what they did or
/// were: a verb in the past tense, or a common short one.
fn is_verb(word: &str) -> bool {
    word.ends_with('l') || ["la", "lo", "li", "ly"].iter().any(|end| word.ends_with(end)) || ["je", "se", "si", "byl", "měl", "dal"].contains(&word)
}

/// The gender of a ruler named `name`: "Alžběta II." is "druhá".
fn regnal_gender(name: &str) -> Gender {
    match gender_before(name) {
        Gender::Feminine => Gender::Feminine,
        _ => Gender::Masculine,
    }
}

/// The value of a Roman numeral written the usual way, 1 to 3999; none
/// for anything else ("IIII", "VX", "MIX" is fine).
fn roman_value(numeral: &str) -> Option<u64> {
    const VALUES: [(u64, &str); 13] =
        [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut rest = numeral;
    let mut value = 0;
    for (n, symbol) in VALUES {
        while let Some(after) = rest.strip_prefix(symbol) {
            value += n;
            rest = after;
        }
    }
    let canonical = {
        let mut left = value;
        let mut written = String::new();
        for (n, symbol) in VALUES {
            while left >= n {
                written.push_str(symbol);
                left -= n;
            }
        }
        written
    };
    (rest.is_empty() && value > 0 && value < 4000 && canonical == numeral).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(text: &str) -> String {
        spoken(text, QuizLanguage::Cs)
    }

    fn en(text: &str) -> String {
        spoken(text, QuizLanguage::En)
    }

    #[test]
    fn czech_numbers_are_words() {
        assert_eq!(cs("V roce 1945 skončila válka."), "V roce tisíc devět set čtyřicet pět skončila válka.");
        assert_eq!(cs("Kolik je 2 + 2?"), "Kolik je dva plus dva?");
        assert_eq!(cs("Má 1 250 obyvatel a 21 000 ovcí."), "Má tisíc dvě stě padesát obyvatel a dvacet jedna tisíc ovcí.");
        assert_eq!(cs("Pí je asi 3,14."), "Pí je asi tři celé čtrnáct.");
        assert_eq!(cs("Agent 007"), "Agent nula nula sedm");
        assert_eq!(cs("Válka trvala 1914–1918."), "Válka trvala tisíc devět set čtrnáct až tisíc devět set osmnáct.");
        assert_eq!(cs("Teplota klesla na -5 °C."), "Teplota klesla na minus pět stupňů Celsia.");
        assert_eq!(cs("2 000 000"), "dva miliony");
    }

    #[test]
    fn a_czech_number_agrees_with_its_unit() {
        assert_eq!(cs("1 km"), "jeden kilometr");
        assert_eq!(cs("2 km"), "dva kilometry");
        assert_eq!(cs("2 Kč"), "dvě koruny");
        assert_eq!(cs("5 Kč"), "pět korun");
        assert_eq!(cs("1 %"), "jedno procento");
        assert_eq!(cs("2,5 km"), "dvě celé pět kilometru");
        assert_eq!(cs("Jel 130 km/h."), "Jel sto třicet kilometrů za hodinu.");
        assert_eq!(cs("Stál 5 mil. Kč."), "Stál pět milionů korun.");
        assert_eq!(cs("Cena je $5 nebo 4 €."), "Cena je pět dolarů nebo čtyři eura.");
        assert_eq!(cs("5 metrů"), "pět metrů", "a word is not a unit");
    }

    #[test]
    fn czech_ordinals_take_the_ending_of_the_word_after_them() {
        assert_eq!(cs("Kdy skončila 2. světová válka?"), "Kdy skončila druhá světová válka?");
        assert_eq!(cs("Ve 14. století"), "Ve čtrnáctém století");
        assert_eq!(cs("Co se stalo po 2. světové válce?"), "Co se stalo po druhé světové válce?");
        assert_eq!(cs("Do 2. světové války"), "Do druhé světové války");
        assert_eq!(cs("ve 3. ročníku a od 19. století"), "ve třetím ročníku a od devatenáctého století");
        assert_eq!(cs("Byl 1. muž na Měsíci."), "Byl první muž na Měsíci.");
        assert_eq!(cs("Kolikátý je 21. ročník?"), "Kolikátý je dvacátý první ročník?");
        assert_eq!(cs("Revoluce začala 17. listopadu 1989."), "Revoluce začala sedmnáctého listopadu tisíc devět set osmdesát devět.");
        assert_eq!(cs("Narodil se 1. 1. 2000 v Brně."), "Narodil se prvního prvního dva tisíce v Brně.");
        assert_eq!(cs("Odpověď je 4. Další otázka."), "Odpověď je čtyři. Další otázka.");
    }

    #[test]
    fn czech_roman_numerals_after_a_name_or_before_a_word_are_ordinals() {
        assert_eq!(cs("Karel IV. založil univerzitu."), "Karel čtvrtý založil univerzitu.");
        assert_eq!(cs("Kdo byl Karel IV.?"), "Kdo byl Karel čtvrtý?");
        assert_eq!(cs("Papež Jan Pavel II"), "Papež Jan Pavel druhý");
        assert_eq!(cs("Ve XX. století"), "Ve dvacátém století");
        assert_eq!(cs("XX. století"), "dvacáté století");
        assert_eq!(cs("Alžběta II. vládla"), "Alžběta druhá vládla");
        assert_eq!(cs("Vitamín C a vitamín D"), "Vitamín C a vitamín D");
        assert_eq!(cs("V roce 1945"), "V roce tisíc devět set čtyřicet pět");
        assert_eq!(cs("MIX"), "MIX", "a word in capitals is not a numeral when nothing says so");
    }

    #[test]
    fn czech_abbreviations_are_written_out() {
        assert_eq!(cs("Např. pes, kočka atd."), "Například pes, kočka a tak dále.");
        assert_eq!(cs("Tzv. sametová revoluce"), "Takzvaná sametová revoluce");
        assert_eq!(cs("tzv. železná opona"), "takzvaná železná opona");
        assert_eq!(cs("Katedrála sv. Víta a kostel sv. Ludmily"), "Katedrála svatého Víta a kostel svaté Ludmily");
        assert_eq!(cs("Sv. Václav"), "Svatý Václav");
        assert_eq!(cs("V roce 500 př. n. l."), "V roce pět set před naším letopočtem.");
        assert_eq!(cs("Má cca 10 m, tj. hodně."), "Má cirka deset metrů, to jest hodně.");
        assert_eq!(cs("atd. Pak"), "a tak dále. Pak");
        assert_eq!(cs("Satd."), "Satd.", "only a whole word is an abbreviation");
    }

    #[test]
    fn a_czech_time_is_its_hours_and_minutes() {
        assert_eq!(cs("Začíná v 12:30."), "Začíná v dvanáct třicet.");
        assert_eq!(cs("v 8:05"), "v osm nula pět");
        assert_eq!(cs("Skóre 3:2"), "Skóre tři:dva");
    }

    #[test]
    fn english_numbers_years_and_ordinals_are_words() {
        assert_eq!(en("In 1945 the war ended."), "In nineteen forty-five the war ended.");
        assert_eq!(en("in 2005 and 2024"), "in two thousand and five and twenty twenty-four");
        assert_eq!(en("1,250 people"), "one thousand two hundred and fifty people");
        assert_eq!(en("It is 3.14 long."), "It is three point one four long.");
        assert_eq!(en("the 21st century"), "the twenty-first century");
        assert_eq!(en("the 2nd and 3rd"), "the second and third");
        assert_eq!(en("It costs $5 or 1 km."), "It costs five dollars or one kilometre.");
        assert_eq!(en("at 7:00"), "at seven o'clock");
        assert_eq!(en("2 + 2 = 4"), "two plus two equals four");
    }

    #[test]
    fn english_roman_numerals_are_rulers_or_counted() {
        assert_eq!(en("Henry VIII had six wives."), "Henry the eighth had six wives.");
        assert_eq!(en("World War II ended in 1945."), "World War two ended in nineteen forty-five.");
        assert_eq!(en("Malcolm X"), "Malcolm X");
        assert_eq!(en("I think so"), "I think so");
    }

    #[test]
    fn english_abbreviations_are_written_out() {
        assert_eq!(en("Dr. Watson, e.g. a doctor"), "Doctor Watson, for example a doctor");
        assert_eq!(en("cats vs. dogs etc."), "cats versus dogs et cetera.");
    }

    #[test]
    fn text_without_anything_to_change_stays_as_it_is() {
        for text in ["Kolik nohou má pavouk?", "Kdo napsal „Babičku“?", "", "AT&T", "C++", "covid-19"] {
            assert_eq!(cs(text), cs(text).as_str());
        }
        assert_eq!(cs("Kolik nohou má pavouk?"), "Kolik nohou má pavouk?");
        assert_eq!(cs("AT&T"), "AT&T");
        assert_eq!(en("C++"), "C++");
    }

    #[test]
    fn letters_are_said_alone() {
        assert_eq!((letter(0, QuizLanguage::Cs), letter(3, QuizLanguage::Cs), letter(1, QuizLanguage::En)), ("Á", "Dé", "B"));
        assert_eq!(letter(6, QuizLanguage::Cs), "");
    }

    #[test]
    fn roman_numerals_are_read_only_when_written_the_usual_way() {
        assert_eq!((roman_value("IV"), roman_value("MCMXLV"), roman_value("XX")), (Some(4), Some(1945), Some(20)));
        assert_eq!((roman_value("IIII"), roman_value("VX"), roman_value("")), (None, None, None));
    }
}
