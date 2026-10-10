//! Numbers written in words: "twelve thousand five hundred", "deux cent
//! mille", "zweihundertfünfzigtausend", "doce mil millones", "一万二千".
//!
//! A number is read around a hundred or a scale word, found by one search,
//! and grows over the number words beside it while they continue one number
//! in one language. A number of one word is read only when it is glued from
//! several ("zwölftausend", "dodicimila"): "mille", "cent" or "thousand"
//! alone is not read. Digits may lead the words ("2 and a half million",
//! "1 millón y medio"); see [`Lead`].

mod words;

use std::ops::Range;
use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, MatchKind};
use rustc_hash::FxHashMap;

use super::digits::digit_value;
use super::scan::is_symbol;
use crate::text::{char_at, char_before, is_letter, is_unspaced_script};
use words::{ALL, DE, FR, Langs, MORPHEMES, WORDS};

/// What a number word stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// A value below a hundred ("twelve", "vingt", "veintiuno").
    Value(u32),
    /// Hundreds written as one word ("doscientos", "quinhentos").
    Hundreds(u32),
    /// A hundred: it multiplies a value before it ("two hundred"), or
    /// stands alone ("cent mille", "cien").
    Hundred,
    /// The power of ten that closes a group ("thousand", "mil", "Millionen").
    Scale(u32),
    /// A word that joins two parts ("and", "et", "y", "e", "und").
    Join,
    /// An article that stands for one before a hundred or a scale ("a").
    Article,
    /// A half: of the scale after it ("half a million", "two and a half
    /// million", "zweieinhalb Millionen"), or of the scale before it ("a
    /// million and a half", "deux millions et demi").
    Half,
    /// One and a half, as one word ("anderthalb").
    OneAndAHalf,
    /// The decimal point a fraction's digits follow ("one point two
    /// million", "zwei Komma fünf Millionen").
    Point,
}

/// A number written in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spelled {
    /// Where it is written.
    pub span: Range<usize>,
    /// Its value.
    pub value: u128,
    /// Opened by an article ("a million", "a hundred thousand"), which prose
    /// writes as an idiom or a round guess ("thanks a million", "a thousand
    /// times") as often as a count.
    pub idiom: bool,
}

/// What a spelled number is found by: a piece of each hundred and scale
/// word, its first letter left out where a capital may stand there
/// ("illion" finds "Millionen"), the glued German and Italian ones inside a
/// word, the short Portuguese and Spanish ones after a space, and the Han
/// numerals a number may begin with. The search is exact, which lets it run
/// on a vector searcher; the word around each find is read in full.
const ANCHOR_PIECES: &[&str] = &[
    "hund", "Hund", "housand", "illion", "liard", "ilion", "illon", "illón", "ilhã", "ilhõ",
    "ilhao", "ilhoe", "ilião", "iliõ", "lakh", "rore", "ausend", "mille", "Mille", "mila", "cent",
    "Cent", "cien", "iento", "ienta", "zent", "nhent", " cem", " mil", "一", "二", "两", "三",
    "四", "五", "六", "七", "八", "九", "十", "百", "千",
];

/// The search for [`ANCHOR_PIECES`]. Leftmost-first matching is what lets
/// it run on the vector searcher, an order of magnitude faster than the
/// standard semantics on a corpus.
static ANCHORS: LazyLock<Option<AhoCorasick>> = LazyLock::new(|| {
    AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(ANCHOR_PIECES)
        .ok()
});

/// The words read whole, by their lowercase form.
static LEXICON: LazyLock<FxHashMap<&'static str, (Part, Langs)>> = LazyLock::new(|| {
    WORDS
        .iter()
        .map(|&(word, part, langs)| (word, (part, langs)))
        .collect()
});

/// The most words one number is read over.
const MAX_WORDS: usize = 16;
/// The most parts a glued word splits into.
const MAX_PARTS: usize = 12;
/// The longest word looked up, in bytes once lowercased.
const MAX_WORD: usize = 48;

/// Whether `c` stands between two words of one number: a space within a
/// line or a hyphen ("twenty-five", "quatre-vingt-dix").
const fn is_joiner(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}' | '-')
}

/// Whether `c` is a space within a line.
const fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{202f}')
}

/// Whether `c` belongs to a word written with spaces.
fn is_word_letter(c: char) -> bool {
    is_letter(c) && !is_unspaced_script(c)
}

/// The start of the word that ends at byte `end`.
fn word_start(text: &str, mut end: usize) -> usize {
    while let Some(c) = char_before(text, end).filter(|&c| is_word_letter(c)) {
        end = end.saturating_sub(c.len_utf8());
    }
    end
}

/// The end of the word that starts at byte `start`.
fn word_end(text: &str, mut start: usize) -> usize {
    while let Some(c) = char_at(text, start).filter(|&c| is_word_letter(c)) {
        start = start.saturating_add(c.len_utf8());
    }
    start
}

/// `word` lowercased into `buf`, None when it does not fit.
fn lower<'b>(word: &str, buf: &'b mut [u8; MAX_WORD]) -> Option<&'b str> {
    let mut len = 0_usize;
    for c in word.chars().flat_map(char::to_lowercase) {
        let mut one = [0_u8; 4];
        let bytes = c.encode_utf8(&mut one).as_bytes();
        let end = len.checked_add(bytes.len())?;
        buf.get_mut(len..end)?.copy_from_slice(bytes);
        len = end;
    }
    std::str::from_utf8(buf.get(..len)?).ok()
}

/// The parts of a lowercase word glued from German or Italian pieces,
/// written to `out` from index `at`, all of one language; how many there
/// are in all, or None when the word is not wholly such pieces.
fn split_glued(
    rest: &str,
    langs: Langs,
    out: &mut [(Part, Langs); MAX_PARTS],
    at: usize,
) -> Option<usize> {
    if rest.is_empty() {
        return (at > 0).then_some(at);
    }
    for &(piece, part, piece_langs) in MORPHEMES {
        let both = langs & piece_langs;
        if both == 0 {
            continue;
        }
        let Some(tail) = rest.strip_prefix(piece) else {
            continue;
        };
        *out.get_mut(at)? = (part, both);
        if let Some(count) = split_glued(tail, both, out, at.saturating_add(1)) {
            return Some(count);
        }
    }
    None
}

/// The parts `word` stands for, written to `out`: one when it is a number
/// word, more when it is glued from pieces; how many, or None when it is
/// not a number word.
fn parts_of(word: &str, out: &mut [(Part, Langs); MAX_PARTS]) -> Option<usize> {
    let mut buf = [0_u8; MAX_WORD];
    let word = lower(word, &mut buf)?;
    if let Some(&found) = LEXICON.get(word) {
        *out.get_mut(0)? = found;
        return Some(1);
    }
    split_glued(word, ALL, out, 0)
}

/// What the parts read so far wait for before they make a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    /// Nothing.
    Nothing,
    /// "a" after a join, which only a half may follow ("two and a half").
    Half,
    /// A half, which the scale after it takes ("two and a half million").
    HalfOfScale,
    /// A decimal point, the digits after it so far, and how many; a scale
    /// takes them ("one point two five million").
    Fraction(u128, u32),
    /// Nothing but a scale of the one just read: a decimal or a half after a
    /// scale ends the number ("deux millions et demi").
    End,
}

/// A spelled number's value as its parts are read, left to right.
#[derive(Debug, Clone, Copy)]
struct Sum {
    /// The groups a scale has closed.
    total: u128,
    /// The group being read, below the next scale.
    group: u128,
    /// The power of the last scale read.
    last_power: Option<u32>,
    /// What the last scale added to `total`.
    last_added: u128,
    /// The last part read.
    last: Option<Part>,
    /// The last value read.
    last_value: u128,
    /// The languages every part so far is written in.
    langs: Langs,
    /// The parts read, joins and an article aside.
    parts: usize,
    /// Whether a hundred or a scale was read.
    multiplied: bool,
    /// Whether it opened with an article.
    article: bool,
    /// What the parts read wait for.
    pending: Pending,
}

impl Sum {
    /// Nothing read yet.
    const fn new() -> Self {
        Self {
            total: 0,
            group: 0,
            last_power: None,
            last_added: 0,
            last: None,
            last_value: 0,
            langs: ALL,
            parts: 0,
            multiplied: false,
            article: false,
            pending: Pending::Nothing,
        }
    }

    /// Reads `part`, written in `langs`; false, the sum unchanged, when it
    /// does not continue the number.
    fn take(&mut self, part: Part, langs: Langs) -> bool {
        let mut next = *self;
        let read = next.read(part, langs).is_some();
        if read {
            *self = next;
        }
        read
    }

    /// [`Sum::take`], changing the sum even when it fails.
    fn read(&mut self, part: Part, langs: Langs) -> Option<()> {
        self.langs &= langs;
        if self.langs == 0 {
            return None;
        }
        let fits = match self.pending {
            Pending::Nothing => true,
            Pending::Half => part == Part::Half,
            Pending::HalfOfScale => matches!(part, Part::Scale(_) | Part::Article),
            Pending::Fraction(..) => matches!(part, Part::Value(_) | Part::Scale(_)),
            Pending::End => matches!(part, Part::Scale(_)),
        };
        if !fits {
            return None;
        }
        let article_waits = self.article && self.parts == 0;
        match part {
            Part::Join => {
                if self.parts == 0 || self.last == Some(Part::Join) {
                    return None;
                }
            }
            Part::Article => self.article()?,
            Part::Half => self.half()?,
            Part::OneAndAHalf => {
                if self.parts > 0 || self.article {
                    return None;
                }
                self.group = 1;
                self.pending = Pending::HalfOfScale;
                self.parts += 1;
            }
            Part::Point => {
                if self.last_power.is_some() || !matches!(self.last, Some(Part::Value(_))) {
                    return None;
                }
                self.pending = Pending::Fraction(0, 0);
            }
            Part::Value(v) => {
                if article_waits {
                    return None;
                }
                if let Pending::Fraction(digits, places) = self.pending {
                    self.pending = fraction_digit(digits, places, v)?;
                    self.parts += 1;
                } else {
                    self.value(u128::from(v))?;
                }
            }
            Part::Hundreds(v) => {
                if article_waits || !self.group.is_multiple_of(1000) {
                    return None;
                }
                self.group = self.group.checked_add(u128::from(v))?;
                self.parts += 1;
                self.multiplied = true;
            }
            Part::Hundred => {
                self.group = match self.group {
                    0 => 100,
                    group if group < 100 => group.checked_mul(100)?,
                    _ => return None,
                };
                self.parts += 1;
                self.multiplied = true;
            }
            Part::Scale(power) => self.scale(power)?,
        }
        self.last = Some(part);
        Some(())
    }

    /// Reads an article: one before a hundred or a scale when it opens the
    /// number ("a million"), "a" before a half after a join ("two and a
    /// half"), or the "a" of "half a million".
    fn article(&mut self) -> Option<()> {
        if self.parts == 0 && !self.article {
            self.article = true;
            self.group = 1;
        } else if self.last == Some(Part::Join) {
            self.pending = Pending::Half;
        } else if self.last != Some(Part::Half) || self.parts != 1 || self.group != 0 {
            return None;
        }
        Some(())
    }

    /// Reads a half: one the next scale takes ("half a million", "a half
    /// million", "un demi-million", "two and a half million", "dos y medio
    /// millones", "zweieinhalb"), or half the scale just read ("a million
    /// and a half", "dos millones y medio").
    fn half(&mut self) -> Option<()> {
        let alone = match self.last {
            None => true,
            Some(Part::Article) if self.parts == 0 => true,
            Some(Part::Value(1)) => self.langs & FR != 0 && self.parts == 1 && self.group == 1,
            _ => false,
        };
        let after_join = matches!(self.last, Some(Part::Article | Part::Join));
        let glued = self.langs & DE != 0 && matches!(self.last, Some(Part::Value(_)));
        if alone {
            self.group = 0;
            self.pending = Pending::HalfOfScale;
        } else if self.group > 0 && (after_join || glued) {
            self.pending = Pending::HalfOfScale;
        } else if self.group == 0 && after_join {
            let half = 10_u128.checked_pow(self.last_power?)? / 2;
            self.total = self.total.checked_add(half)?;
            self.pending = Pending::End;
        } else {
            return None;
        }
        self.parts += 1;
        Some(())
    }

    /// Reads a value below a hundred: it fills the group's empty tens or
    /// units ("twenty five", "cent vingt"), a French teen follows sixty or
    /// eighty ("soixante-dix"), "vingt" after "quatre" makes eighty, and a
    /// German ten follows the unit it is joined to ("fünfundzwanzig").
    fn value(&mut self, v: u128) -> Option<()> {
        let low = self.group % 100;
        let french = self.langs & FR != 0;
        let quatre_vingt = french && low == 4 && v == 20 && self.last == Some(Part::Value(4));
        let german_ten = self.langs & DE != 0
            && self.last == Some(Part::Join)
            && self.last_value < 10
            && v >= 20
            && v.is_multiple_of(10);
        let fits = low == 0
            || (low.is_multiple_of(10) && v < 10)
            || (french && (low == 60 || low == 80) && (10..20).contains(&v))
            || quatre_vingt
            || german_ten;
        if !fits {
            return None;
        }
        let added = if quatre_vingt { 76 } else { v };
        self.group = self.group.checked_add(added)?;
        self.last_value = v;
        self.parts += 1;
        Some(())
    }

    /// Reads a scale of `power`: it closes the group, or multiplies the
    /// scale right before it ("mil millones", "thousand million").
    fn scale(&mut self, power: u32) -> Option<()> {
        let scale = 10_u128.checked_pow(power)?;
        match (self.last, self.last_power) {
            (Some(Part::Scale(_)), Some(last_power)) if power > last_power => {
                let added = self.last_added.checked_mul(scale)?;
                self.total = self
                    .total
                    .checked_sub(self.last_added)?
                    .checked_add(added)?;
                self.last_added = added;
                self.last_power = Some(last_power.checked_add(power)?);
            }
            (_, Some(last_power)) if power >= last_power => return None,
            _ => {
                let added = match self.pending {
                    Pending::Fraction(digits, places) => {
                        let shift = power.checked_sub(places).filter(|_| places > 0)?;
                        self.pending = Pending::End;
                        self.group
                            .checked_mul(scale)?
                            .checked_add(digits.checked_mul(10_u128.checked_pow(shift)?)?)?
                    }
                    Pending::HalfOfScale => {
                        self.pending = Pending::Nothing;
                        self.group.checked_mul(scale)?.checked_add(scale / 2)?
                    }
                    Pending::Nothing | Pending::Half | Pending::End => {
                        let multiplier = match self.group {
                            0 if self.parts == 0 => 1,
                            0 => return None,
                            group => group,
                        };
                        multiplier.checked_mul(scale)?
                    }
                };
                self.total = self.total.checked_add(added)?;
                self.group = 0;
                self.last_power = Some(power);
                self.last_added = added;
            }
        }
        self.parts += 1;
        self.multiplied = true;
        Some(())
    }

    /// Whether the parts read make a number: nothing waits, and the last
    /// part is not a join.
    fn complete(&self) -> bool {
        self.last != Some(Part::Join) && matches!(self.pending, Pending::Nothing | Pending::End)
    }

    /// The value read, when it is a number of its own: a hundred or a scale
    /// in it, and two parts at least.
    fn finish(&self) -> Option<(u128, bool)> {
        let words = self.parts.saturating_add(usize::from(self.article));
        if !self.multiplied || words < 2 || !self.complete() {
            return None;
        }
        let value = self.total.checked_add(self.group)?;
        Some((value, self.article))
    }
}

/// The fraction `digits` with `places` digits, once the digit word `v`
/// follows it; None when `v` is not a digit.
fn fraction_digit(digits: u128, places: u32, v: u32) -> Option<Pending> {
    if v >= 10 || places >= 6 {
        return None;
    }
    let digits = digits.checked_mul(10)?.checked_add(u128::from(v))?;
    Some(Pending::Fraction(digits, places.checked_add(1)?))
}

/// Whether `text` holds a number word at `start..end`.
fn is_number_word(text: &str, start: usize, end: usize) -> bool {
    let mut parts = [(Part::Join, 0); MAX_PARTS];
    start < end
        && text
            .get(start..end)
            .is_some_and(|word| parts_of(word, &mut parts).is_some())
}

/// What stands before the first word a number may run over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lead {
    /// No figure in digits.
    Words,
    /// A whole figure of one to three digits, after a space, which the
    /// words continue ("2 and a half million", "3 thousand five hundred");
    /// it is the first word, and the value here.
    Digits(u32),
    /// Another figure in digits, after a space ("1.5 million"), whose scale
    /// word a number in words does not start with.
    Figure,
}

/// The figure in digits that ends at byte `end`, before the first word of a
/// number, none of it before byte `floor`: where it starts and what it is.
fn digits_before(text: &str, end: usize, floor: usize) -> (usize, Lead) {
    let mut start = end;
    let mut value = 0_u32;
    let mut place = 1_u32;
    let mut count = 0_usize;
    while let Some(c) = char_before(text, start) {
        let Some(digit) = digit_value(c) else {
            break;
        };
        value = place
            .checked_mul(u32::from(digit))
            .and_then(|d| value.checked_add(d))
            .unwrap_or(u32::MAX);
        place = place.saturating_mul(10);
        count += 1;
        start = start.saturating_sub(c.len_utf8());
    }
    // The run is a whole figure unless a word or a group before it goes on.
    let whole = char_before(text, start).is_none_or(|c| {
        let grouped = is_symbol(c)
            && char_before(text, start.saturating_sub(c.len_utf8()))
                .is_some_and(|d| digit_value(d).is_some());
        !(grouped || is_letter(c) || matches!(c, '_' | '.' | ','))
    });
    if count == 0 {
        (end, Lead::Words)
    } else if whole && start >= floor && (1..=3).contains(&count) && value > 0 {
        (start, Lead::Digits(value))
    } else {
        (end, Lead::Figure)
    }
}

/// The words a number may run over around the word `anchor`, in order,
/// none before byte `floor`: the anchor and the number words joined to it on
/// either side, and the figure in digits a space before them. Returns how
/// many words there are, the anchor's index, and what leads them.
fn words_around(
    text: &str,
    anchor: Range<usize>,
    floor: usize,
    out: &mut [(usize, usize); MAX_WORDS],
) -> (usize, usize, Lead) {
    let mut before = [(0_usize, 0_usize); MAX_WORDS];
    let mut count_before = 0_usize;
    let mut first = anchor.start;
    let mut lead = Lead::Words;
    while count_before < MAX_WORDS / 2 {
        let Some(joiner) = char_before(text, first).filter(|&c| is_joiner(c)) else {
            break;
        };
        let end = first.saturating_sub(joiner.len_utf8());
        let start = word_start(text, end);
        if start == end && is_space(joiner) {
            let (digits_start, found) = digits_before(text, end, floor);
            lead = found;
            if let (Lead::Digits(_), Some(slot)) = (found, before.get_mut(count_before)) {
                *slot = (digits_start, end);
                count_before += 1;
            }
            break;
        }
        if start < floor || !is_number_word(text, start, end) {
            break;
        }
        if let Some(slot) = before.get_mut(count_before) {
            *slot = (start, end);
        }
        count_before += 1;
        first = start;
    }
    let mut count = 0_usize;
    for &word in before.iter().take(count_before).rev() {
        if let Some(slot) = out.get_mut(count) {
            *slot = word;
            count += 1;
        }
    }
    let anchor_index = count;
    if let Some(slot) = out.get_mut(count) {
        *slot = (anchor.start, anchor.end);
        count += 1;
    }
    let mut last = anchor.end;
    while count < MAX_WORDS {
        let Some(joiner) = char_at(text, last).filter(|&c| is_joiner(c)) else {
            break;
        };
        let start = last.saturating_add(joiner.len_utf8());
        let end = word_end(text, start);
        if !is_number_word(text, start, end) {
            break;
        }
        if let Some(slot) = out.get_mut(count) {
            *slot = (start, end);
        }
        count += 1;
        last = end;
    }
    (count, anchor_index, lead)
}

/// Reads the words `words[from..]` as one number from their first, the
/// first standing for `digits` when they lead it: the sum and the index past
/// the last word it takes, what the number has to wait for left out; and the
/// index of the first word it could not take.
fn read_from(
    text: &str,
    words: &[(usize, usize)],
    from: usize,
    digits: Option<u32>,
) -> (Sum, usize, usize) {
    let mut sum = Sum::new();
    let mut good = (sum, from);
    let mut parts = [(Part::Join, 0); MAX_PARTS];
    for (i, &(start, end)) in words.iter().enumerate().skip(from) {
        let count = match (digits, parts.first_mut()) {
            (Some(value), Some(first)) if i == from => {
                *first = (Part::Value(value), ALL);
                1
            }
            _ => text
                .get(start..end)
                .and_then(|w| parts_of(w, &mut parts))
                .unwrap_or(0),
        };
        let took = count > 0
            && parts
                .iter()
                .take(count)
                .all(|&(part, langs)| sum.take(part, langs));
        if !took {
            return (good.0, good.1, i);
        }
        if sum.complete() {
            good = (sum, i.saturating_add(1));
        }
    }
    (good.0, good.1, words.len())
}

/// The spelled number around the anchor match `found`, none of it before
/// byte `floor`.
fn phrase_at(text: &str, found: Range<usize>, floor: usize) -> Option<Spelled> {
    let start = word_start(text, found.start);
    let end = word_end(text, found.end);
    if start < floor {
        return None;
    }
    let mut parts = [(Part::Join, 0); MAX_PARTS];
    parts_of(text.get(start..end)?, &mut parts)?;
    let mut words = [(0_usize, 0_usize); MAX_WORDS];
    let (count, anchor, lead) = words_around(text, start..end, floor, &mut words);
    let words = words.get(..count)?;
    let mut from = match lead {
        Lead::Figure if starts_with_multiplier(text, words) => 1,
        Lead::Words | Lead::Digits(_) | Lead::Figure => 0,
    };
    loop {
        let digits = match lead {
            Lead::Digits(value) if from == 0 => Some(value),
            Lead::Digits(_) | Lead::Words | Lead::Figure => None,
        };
        let (sum, taken, failed) = read_from(text, words, from, digits);
        if taken > anchor {
            // Digits and one scale word are the figure's own ("3 thousand").
            if digits.is_some() && taken < 3 {
                return None;
            }
            let (value, idiom) = sum.finish()?;
            let &(first, _) = words.get(from)?;
            let &(_, last) = words.get(taken.checked_sub(1)?)?;
            return Some(Spelled {
                span: first..last,
                value,
                idiom,
            });
        }
        if failed > anchor {
            return None;
        }
        from = failed.max(from.saturating_add(1));
    }
}

/// Whether the first of `words` is a hundred or a scale word, which a
/// figure in digits before it takes as its own scale.
fn starts_with_multiplier(text: &str, words: &[(usize, usize)]) -> bool {
    let mut parts = [(Part::Join, 0); MAX_PARTS];
    words
        .first()
        .and_then(|&(start, end)| text.get(start..end))
        .and_then(|word| parts_of(word, &mut parts))
        .is_some_and(|_| matches!(parts.first(), Some((Part::Hundred | Part::Scale(_), _))))
}

/// What a Han numeral stands for.
enum Han {
    /// A digit; zero is a placeholder between parts ("一万零五").
    Digit(u128),
    /// A multiplier within a myriad: ten, a hundred, a thousand.
    Small(u32),
    /// A myriad and above: 万, 億, 兆.
    Large(u32),
}

/// The Han numeral `c` is, if any.
const fn han(c: char) -> Option<Han> {
    Some(match c {
        '〇' | '零' => Han::Digit(0),
        '一' => Han::Digit(1),
        '二' | '两' => Han::Digit(2),
        '三' => Han::Digit(3),
        '四' => Han::Digit(4),
        '五' => Han::Digit(5),
        '六' => Han::Digit(6),
        '七' => Han::Digit(7),
        '八' => Han::Digit(8),
        '九' => Han::Digit(9),
        '十' => Han::Small(1),
        '百' => Han::Small(2),
        '千' => Han::Small(3),
        '万' | '萬' => Han::Large(4),
        '億' | '亿' => Han::Large(8),
        '兆' => Han::Large(12),
        _ => return None,
    })
}

/// The Han numeral that starts at byte `start` ("一万二千", "三百", "十二万"),
/// read while its characters continue it. A run right after a digit is a
/// figure's scale ("5千万") and is left to the digits.
fn han_at(text: &str, start: usize) -> Option<Spelled> {
    if char_before(text, start).is_some_and(|c| digit_value(c).is_some()) {
        return None;
    }
    let mut total = 0_u128;
    let mut section = 0_u128;
    let mut digit: Option<u128> = None;
    let mut last_small: Option<u32> = None;
    let mut last_large: Option<u32> = None;
    let mut multiplied = false;
    let mut chars = 0_usize;
    let mut end = start;
    while let Some(c) = char_at(text, end) {
        let Some(numeral) = han(c) else {
            break;
        };
        match numeral {
            Han::Digit(0) if chars > 0 => digit = None,
            Han::Digit(0) => return None,
            Han::Digit(d) => {
                if digit.is_some() {
                    break;
                }
                digit = Some(d);
            }
            Han::Small(power) => {
                if last_small.is_some_and(|last| power >= last) {
                    break;
                }
                let times = digit.take().unwrap_or(1);
                section = section.checked_add(times.checked_mul(10_u128.checked_pow(power)?)?)?;
                last_small = Some(power);
                multiplied = true;
            }
            Han::Large(power) => {
                if chars == 0 || last_large.is_some_and(|last| power >= last) {
                    break;
                }
                let group = section.checked_add(digit.take().unwrap_or(0))?;
                if group == 0 {
                    break;
                }
                total = total.checked_add(group.checked_mul(10_u128.checked_pow(power)?)?)?;
                section = 0;
                last_small = None;
                last_large = Some(power);
                multiplied = true;
            }
        }
        chars += 1;
        end = end.saturating_add(c.len_utf8());
    }
    if chars < 2 || !multiplied {
        return None;
    }
    let value = total
        .checked_add(section)?
        .checked_add(digit.unwrap_or(0))?;
    Some(Spelled {
        span: start..end,
        value,
        idiom: false,
    })
}

/// The numbers `text` writes in words, in order of appearance.
#[derive(Debug)]
pub struct SpelledNumbers<'a> {
    /// The text read.
    text: &'a str,
    /// The anchor pieces not yet read; None when the search did not build.
    anchors: Option<aho_corasick::FindIter<'static, 'a>>,
    /// Where the last number read ends, so an anchor inside it is skipped.
    consumed: usize,
}

/// The numbers `text` writes in words.
pub fn spelled(text: &str) -> SpelledNumbers<'_> {
    SpelledNumbers {
        text,
        anchors: ANCHORS.as_ref().map(|anchors| anchors.find_iter(text)),
        consumed: 0,
    }
}

impl Iterator for SpelledNumbers<'_> {
    type Item = Spelled;

    fn next(&mut self) -> Option<Spelled> {
        loop {
            let found = self.anchors.as_mut()?.next()?;
            let start = if self.text.as_bytes().get(found.start()) == Some(&b' ') {
                found.start().saturating_add(1)
            } else {
                found.start()
            };
            if start < self.consumed {
                continue;
            }
            let number = if char_at(self.text, start).is_some_and(|c| han(c).is_some()) {
                han_at(self.text, start)
            } else {
                phrase_at(self.text, start..found.end(), self.consumed)
            };
            if let Some(number) = number {
                self.consumed = number.span.end;
                return Some(number);
            }
        }
    }
}
