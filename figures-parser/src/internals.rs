//! The internals kind: the names of an agent's own machinery a reply shows
//! (its tools, the functions its sandbox exposes, its memory files, the
//! directories and variables its harness runs under), and a reply with its
//! memory layout put in plain words.

use std::borrow::Cow;

use rustc_hash::FxHashMap;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::{UWordBounds, UnicodeSegmentation};

use crate::text::is_unspaced_script;

/// The names a reply must not show unless a person wrote them; the caller
/// gives its own, as [`internal_names`] reads them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vocabulary<'a> {
    /// The tools and sandbox functions, by name. One with an underscore
    /// (`web_search`) counts wherever a word spells it; one without
    /// ("learn") only where it is written as a call, followed by `(`.
    pub names: &'a [&'a str],
    /// Memory file names ("workspace.md"), counted as whole words. A tool
    /// result that holds one vouches for it, since a repository has its own.
    pub files: &'a [&'a str],
    /// Directories ("/memory/", "/sdk/"), counted only where they begin a
    /// path: one inside a longer path is someone else's.
    pub roots: &'a [&'a str],
    /// Other names (programs, environment variables), counted anywhere,
    /// whatever printed them, since code a run executes can print any.
    pub fixed: &'a [&'a str],
}

/// What a path or a link is made of: a root after one of these sits inside
/// someone else's path.
const fn is_path_char(c: char) -> bool {
    c.is_ascii_lowercase()
        || c.is_ascii_digit()
        || matches!(c, '-' | '.' | '_' | '~' | '%' | '/' | ':')
}

/// What a file name is made of.
const fn is_file_name_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_' | '.')
}

/// What an identifier is made of.
const fn is_word_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
}

/// Punctuation that ends a sentence, not a path.
const fn is_trailing(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}')
}

/// `c` lowercased, when that is one character: what a class of ASCII
/// characters is tested against, so the Kelvin sign reads as `k` and a
/// capital that lowercases to two characters reads as neither.
fn lower_one(c: char) -> Option<char> {
    let mut lower = c.to_lowercase();
    let first = lower.next()?;
    lower.next().is_none().then_some(first)
}

/// Whether `c`, lowercased to one character, is in `class`.
fn lower_in(c: char, class: fn(char) -> bool) -> bool {
    lower_one(c).is_some_and(class)
}

/// `text` lowercased, as a whole: a final sigma reads as one.
fn lowered(text: &str) -> Cow<'_, str> {
    if text
        .bytes()
        .any(|b| b.is_ascii_uppercase() || !b.is_ascii())
    {
        Cow::Owned(text.to_lowercase())
    } else {
        Cow::Borrowed(text)
    }
}

/// `word` lowercased into `out`.
fn lower_into(word: &str, out: &mut String) {
    out.clear();
    if word.is_ascii() {
        out.extend(word.chars().map(|c| c.to_ascii_lowercase()));
    } else {
        out.push_str(&word.to_lowercase());
    }
}

/// Whether a word segment counts as a word: it holds a letter, a digit or
/// an ideograph, as a segmenter's word-like segments do.
fn is_word_like(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

/// Whether `c` is Hangul, which Korean glues a particle to a word with
/// ("workspace.md에").
fn is_hangul(c: char) -> bool {
    c.script() == Script::Hangul
}

/// The word segments of `text`, as the Unicode word rules part them, with
/// two repairs. Hangul is parted from the letters of another script: the
/// rules join a Hangul syllable to a Latin letter, while Korean writes a
/// particle right after a name ("workspace.md에"), which a dictionary-based
/// segmenter parts. And a mark joined to an emoji by a zero-width joiner
/// (".\u{200d}😀") is parted from the word before it, as the rules part it.
fn word_segments(text: &str) -> WordSegments<'_> {
    WordSegments {
        bounds: text.split_word_bounds(),
        runs: "",
        hangul: false,
        inner: None,
    }
}

/// The iterator [`word_segments`] returns.
struct WordSegments<'t> {
    /// The text's segments, as the word rules part them.
    bounds: UWordBounds<'t>,
    /// What is left of a segment that needs parting.
    runs: &'t str,
    /// Whether that segment mixes Hangul with another script.
    hangul: bool,
    /// The segments of the current run of that segment.
    inner: Option<UWordBounds<'t>>,
}

/// Whether `segment` mixes Hangul with letters or digits of another script.
fn mixes_hangul(segment: &str) -> bool {
    segment.chars().any(is_hangul)
        && segment
            .chars()
            .any(|c| c.is_alphanumeric() && !is_hangul(c))
}

/// Whether `c` is an emoji or another pictograph a joiner ties into a
/// sequence.
fn is_pictograph(c: char) -> bool {
    get_general_category(c) == GeneralCategory::OtherSymbol
}

/// The places in `text` where a mark that is no letter or digit is tied to
/// a pictograph by a zero-width joiner: the start of the marks, and the end
/// of the pictograph.
fn joined_marks(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut chars = text.char_indices().peekable();
    let mut marks_from: Option<usize> = None;
    std::iter::from_fn(move || {
        while let Some((i, c)) = chars.next() {
            if c.is_alphanumeric() {
                marks_from = None;
                continue;
            }
            let start = *marks_from.get_or_insert(i);
            if c != '\u{200d}' || start == i {
                continue;
            }
            if let Some(&(at, n)) = chars.peek()
                && is_pictograph(n)
            {
                marks_from = None;
                return Some((start, at.saturating_add(n.len_utf8())));
            }
        }
        None
    })
}

/// The first place past the start of `text` where a run ends: a change
/// between Hangul and another script (when `hangul`), the start of the
/// marks a joiner ties to a pictograph, or the end of that pictograph.
fn next_cut(text: &str, hangul: bool) -> Option<usize> {
    let first = text.chars().next()?;
    let script_cut = hangul
        .then(|| {
            text.char_indices()
                .find(|&(_, c)| is_hangul(c) != is_hangul(first) && c.script() != Script::Inherited)
                .map(|(i, _)| i)
        })
        .flatten();
    let joiner_cut = joined_marks(text)
        .flat_map(<[usize; 2]>::from)
        .find(|&cut| cut > 0 && cut < text.len());
    [script_cut, joiner_cut].into_iter().flatten().min()
}

impl<'t> Iterator for WordSegments<'t> {
    type Item = &'t str;

    fn next(&mut self) -> Option<&'t str> {
        loop {
            if let Some(segment) = self.inner.as_mut().and_then(Iterator::next) {
                return Some(segment);
            }
            self.inner = None;
            if !self.runs.is_empty() {
                let len = next_cut(self.runs, self.hangul).unwrap_or(self.runs.len());
                let (run, rest) = self.runs.split_at_checked(len)?;
                self.runs = rest;
                self.inner = Some(run.split_word_bounds());
                continue;
            }
            let segment = self.bounds.next()?;
            let hangul = mixes_hangul(segment);
            if hangul || (segment.len() > 1 && joined_marks(segment).next().is_some()) {
                self.runs = segment;
                self.hangul = hangul;
                continue;
            }
            return Some(segment);
        }
    }
}

/// Whether `c` belongs to a script the Unicode word rules leave to a
/// dictionary (Thai, Lao, Khmer, Myanmar and the Tai scripts), whose letters
/// they part one from the next.
fn is_dictionary_script(c: char) -> bool {
    matches!(
        c.script(),
        Script::Thai
            | Script::Lao
            | Script::Khmer
            | Script::Myanmar
            | Script::Tai_Tham
            | Script::Tai_Viet
            | Script::New_Tai_Lue
            | Script::Tai_Le
    )
}

/// Whether `c` is a combining mark, which belongs to the letter before it.
fn is_mark(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark
    )
}

/// Whether `c`, standing beside a name, continues the word the name is in:
/// a digit, an underscore or a letter of a script that spaces its words. A
/// letter of a script written without spaces sets a name apart, as a word
/// of its own would.
fn continues_word(c: char) -> bool {
    c == '_' || (c.is_alphanumeric() && !is_unspaced_script(c))
}

/// Whether `name` stands whole in `text` (both lowercased) at byte `at`:
/// nothing before it continues its word, once the marks of the letter
/// before are passed, and nothing after it does, a mark included.
fn whole_at(text: &str, at: usize, name: &str) -> bool {
    let head = text.get(..at).unwrap_or_default();
    let before = head.chars().rev().find(|&c| !is_mark(c));
    let after = text
        .get(at.saturating_add(name.len())..)
        .and_then(|rest| rest.chars().next());
    before.is_none_or(|c| !continues_word(c))
        && after.is_none_or(|c| !is_mark(c) && !continues_word(c))
}

/// The names of a list that a text spells, lowercased, each once; built
/// once per list.
struct Spelled {
    /// Each name, lowercased, to its index.
    index: FxHashMap<String, usize>,
    /// The names in order, lowercased.
    names: Vec<String>,
    /// The names that hold a letter of a dictionary script, which the word
    /// rules never keep in one segment, so they are looked for as written.
    dictionary: Vec<usize>,
}

/// Which names of a [`Spelled`] list one text spells.
struct Found {
    /// Spelled as a word.
    word: Vec<bool>,
    /// Spelled as a call: the word right before a segment that opens on `(`.
    call: Vec<bool>,
}

impl Spelled {
    /// The list, lowercased, each name once.
    fn new<'n>(names: impl Iterator<Item = &'n str>) -> Self {
        let mut index = FxHashMap::default();
        let mut list = Vec::new();
        let mut dictionary = Vec::new();
        for name in names {
            let lower = name.to_lowercase();
            if !index.contains_key(&lower) {
                if lower.chars().any(is_dictionary_script) {
                    dictionary.push(list.len());
                }
                index.insert(lower.clone(), list.len());
                list.push(lower);
            }
        }
        Self {
            index,
            names: list,
            dictionary,
        }
    }

    /// Which of the names `text` spells, reading its word segments as the
    /// Unicode word rules part them.
    fn found(&self, text: &str) -> Found {
        let mut found = Found {
            word: vec![false; self.names.len()],
            call: vec![false; self.names.len()],
        };
        if self.names.is_empty() {
            return found;
        }
        let mut scratch = String::new();
        let mut previous: Option<usize> = None;
        for segment in word_segments(text) {
            if segment.starts_with('(')
                && let Some(slot) = previous.and_then(|i| found.call.get_mut(i))
            {
                *slot = true;
            }
            previous = None;
            if is_word_like(segment) {
                lower_into(segment, &mut scratch);
                if let Some(&i) = self.index.get(scratch.as_str()) {
                    previous = Some(i);
                    if let Some(slot) = found.word.get_mut(i) {
                        *slot = true;
                    }
                }
            }
        }
        if !self.dictionary.is_empty() {
            self.find_written(&lowered(text), &mut found);
        }
        found
    }

    /// Marks the dictionary-script names that `text` (lowercased) holds
    /// whole, as a word, and as a call where `(` follows.
    fn find_written(&self, text: &str, found: &mut Found) {
        for &i in &self.dictionary {
            let Some(name) = self.names.get(i) else {
                continue;
            };
            for at in occurrences(text, name) {
                if !whole_at(text, at, name) {
                    continue;
                }
                if let Some(slot) = found.word.get_mut(i) {
                    *slot = true;
                }
                let after = text.get(at.saturating_add(name.len())..);
                if after.is_some_and(|rest| rest.starts_with('('))
                    && let Some(slot) = found.call.get_mut(i)
                {
                    *slot = true;
                }
            }
        }
    }
}

impl Found {
    /// Whether the text names the `i`th name: as a word when it has an
    /// underscore, as a call when it has none.
    fn names(&self, i: usize, name: &str) -> bool {
        let spelled = if name.contains('_') {
            &self.word
        } else {
            &self.call
        };
        spelled.get(i).copied().unwrap_or(false)
    }

    /// Whether the text spells the `i`th name as a word.
    fn word(&self, i: usize) -> bool {
        self.word.get(i).copied().unwrap_or(false)
    }
}

/// Whether `text` (lowercased) names `name` (lowercased): a root only where
/// it begins a path, any other name anywhere.
fn mentions(text: &str, name: &str, root: bool) -> bool {
    if !root {
        return text.contains(name);
    }
    occurrences(text, name)
        .any(|at| crate::text::char_before(text, at).is_none_or(|c| !is_path_char(c)))
}

/// Every start of `name` in `text`, overlapping ones included.
fn occurrences<'t>(text: &'t str, name: &'t str) -> impl Iterator<Item = usize> + 't {
    let step = name.chars().next().map_or(1, char::len_utf8);
    let mut from: usize = 0;
    std::iter::from_fn(move || {
        if name.is_empty() {
            return None;
        }
        let at = from.checked_add(text.get(from..)?.find(name)?)?;
        from = at.checked_add(step)?;
        Some(at)
    })
}

/// Whether `text` holds the file name `name` (lowercase) whole, case aside,
/// not as part of a longer one.
fn holds_file(text: &str, name: &str) -> bool {
    let count = name.chars().count();
    let mut from = 0;
    while let Some(at) = find_ignoring_case(text, name, from) {
        let end = at.saturating_add(chars_len(text, at, count));
        let before = crate::text::char_before(text, at);
        let after = crate::text::char_at(text, end);
        // A neighbour is read as the text lowercased whole reads it: by the
        // end of its lowercase that touches the name.
        if before.is_none_or(|c| !c.to_lowercase().last().is_some_and(is_file_name_char))
            && after.is_none_or(|c| !c.to_lowercase().next().is_some_and(is_file_name_char))
        {
            return true;
        }
        from = at.saturating_add(crate::text::char_at(text, at).map_or(1, char::len_utf8));
    }
    false
}

/// The internals check for one turn: the person's words and the names, read
/// once, for every reply of the turn [`InternalsCheck::names_in`] checks.
pub struct InternalsCheck<'v> {
    /// The code names, lowercased, that the person did not write.
    names: Spelled,
    /// The memory file names, lowercased.
    files: Spelled,
    /// Whether the person wrote each file name.
    files_said: Vec<bool>,
    /// The roots and fixed names the person did not write, as given, each
    /// lowercased and marked a root or not.
    fixed: Vec<(&'v str, String, bool)>,
}

impl<'v> InternalsCheck<'v> {
    /// The check for a turn whose people wrote `said`.
    #[must_use]
    pub fn new(said: &str, vocabulary: &Vocabulary<'v>) -> Self {
        let lower_said = lowered(said);
        let all = Spelled::new(vocabulary.names.iter().copied());
        let in_said = all.found(said);
        let names = Spelled::new(
            all.names
                .iter()
                .enumerate()
                .filter(|(i, name)| !in_said.names(*i, name))
                .map(|(_, name)| name.as_str()),
        );
        let files = Spelled::new(vocabulary.files.iter().copied());
        let files_said = files.found(said).word;
        Self {
            names,
            files,
            files_said,
            fixed: vocabulary
                .roots
                .iter()
                .map(|&name| (name, true))
                .chain(vocabulary.fixed.iter().map(|&name| (name, false)))
                .map(|(name, root)| (name, name.to_lowercase(), root))
                .filter(|(_, lower, root)| !mentions(&lower_said, lower, *root))
                .collect(),
        }
    }

    /// The names of the agent's internals `reply` shows that the person did
    /// not write: code names (lowercased), then memory files no tool result
    /// in `vouching` holds whole (lowercased), then roots and fixed names, as
    /// given. `vouching` (the tool results other than the memory tools',
    /// which list the files themselves) is read only when the reply names a
    /// file.
    #[must_use]
    pub fn names_in<'s, V: AsRef<str>>(
        &'s self,
        reply: &str,
        vouching: impl FnOnce() -> V,
    ) -> Vec<&'s str> {
        let in_reply = self.names.found(reply);
        let mut out: Vec<&str> = self
            .names
            .names
            .iter()
            .enumerate()
            .filter(|(i, name)| in_reply.names(*i, name))
            .map(|(_, name)| name.as_str())
            .collect();
        let files_in_reply = self.files.found(reply);
        let named: Vec<&str> = self
            .files
            .names
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                files_in_reply.word(*i) && !self.files_said.get(*i).copied().unwrap_or(false)
            })
            .map(|(_, name)| name.as_str())
            .collect();
        if !named.is_empty() {
            let vouching = vouching();
            let vouching = vouching.as_ref();
            out.extend(named.into_iter().filter(|name| !holds_file(vouching, name)));
        }
        let lower_reply = lowered(reply);
        out.extend(
            self.fixed
                .iter()
                .filter(|(_, lower, root)| mentions(&lower_reply, lower, *root))
                .map(|(name, _, _)| *name),
        );
        out
    }
}

/// The names of an agent's own internals `reply` shows that no person wrote
/// in `said`.
///
/// They are its tool and function names, its memory files, the
/// directories and names its harness runs under. A memory file passes when
/// `vouching`, the tool results other than the memory tools', holds it
/// whole: a repository's own `workspace.md` is the person's to see.
///
/// Built for one reply; [`InternalsCheck`] reads the person's words once for
/// every reply of a turn.
///
/// # Examples
///
/// ```
/// use figures_parser::{Vocabulary, internal_names};
///
/// let vocabulary = Vocabulary {
///     names: &["web_search", "learn"],
///     files: &["workspace.md"],
///     roots: &["/memory/"],
///     fixed: &["RUN_TOKEN"],
/// };
/// let reply = "I called `learn()` and web_search; notes live in /memory/workspace.md.";
/// assert_eq!(
///     internal_names(reply, "", &vocabulary, ""),
///     ["web_search", "learn", "workspace.md", "/memory/"],
/// );
/// // A word that is also a tool's name counts only as a call.
/// assert!(internal_names("Happy to help you learn it.", "", &vocabulary, "").is_empty());
/// ```
#[must_use]
pub fn internal_names(
    reply: &str,
    said: &str,
    vocabulary: &Vocabulary<'_>,
    vouching: &str,
) -> Vec<String> {
    InternalsCheck::new(said, vocabulary)
        .names_in(reply, || vouching)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Where an agent keeps its memory, and the plain words each part reads as;
/// [`scrub_memory_layout`] writes them in. Names are matched case aside and
/// given in lowercase.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryLayout<'a> {
    /// The memory directory, with its trailing slash ("/memory/").
    pub root: &'a str,
    /// The prefix every memory tool's name carries ("memory_").
    pub tool_prefix: &'a str,
    /// A folder under the root and what a path inside it reads as
    /// (`("integrations/", "the integration notes")`), checked first.
    pub folders: &'a [(&'a str, &'a str)],
    /// A file name and what a path ending in it, or the name alone, reads as
    /// (`("user.md", "your notes")`), checked in order.
    pub files: &'a [(&'a str, &'a str)],
    /// What the root alone reads as ("memory").
    pub root_phrase: &'a str,
    /// What any other path under the root reads as ("the team's notes").
    pub other_phrase: &'a str,
    /// What a memory tool's name reads as ("my memory").
    pub tool_phrase: &'a str,
}

impl MemoryLayout<'_> {
    /// The root without its trailing slash, so the bare directory is caught.
    fn prefix(&self) -> &str {
        self.root.strip_suffix('/').unwrap_or(self.root)
    }

    /// What `path` (lowercased) reads as.
    fn phrase(&self, path: &str) -> &str {
        let under = path.strip_prefix(self.root);
        if let Some(&(_, phrase)) = self
            .folders
            .iter()
            .find(|(folder, _)| under.is_some_and(|rest| rest.starts_with(folder)))
        {
            return phrase;
        }
        if let Some(&(_, phrase)) = self.files.iter().find(|(file, _)| path.ends_with(file)) {
            return phrase;
        }
        if path == self.root || path == self.prefix() {
            self.root_phrase
        } else {
            self.other_phrase
        }
    }
}

/// The next start of `needle` (lowercase) in `text` from byte `from`, case
/// aside, compared character by character so every offset stays one of
/// `text`'s: a character that lowercases to more than one never matches.
fn find_ignoring_case(text: &str, needle: &str, from: usize) -> Option<usize> {
    let first = needle.chars().next()?;
    let rest = text.get(from..)?;
    rest.char_indices()
        .filter(|&(_, c)| lower_one(c) == Some(first))
        .map(|(at, _)| at)
        .find(|&at| {
            let mut chars = rest.get(at..).into_iter().flat_map(str::chars);
            needle
                .chars()
                .all(|n| chars.next().and_then(lower_one) == Some(n))
        })
        .and_then(|at| from.checked_add(at))
}

/// The byte length of the first `count` characters of `text` from `at`.
fn chars_len(text: &str, at: usize, count: usize) -> usize {
    text.get(at..)
        .into_iter()
        .flat_map(str::chars)
        .take(count)
        .map(char::len_utf8)
        .sum()
}

/// The end of the run of characters `class` accepts, lowercased, from `at`.
fn run_end(text: &str, at: usize, class: fn(char) -> bool) -> usize {
    let rest = text.get(at..).unwrap_or_default();
    let len = rest
        .char_indices()
        .find(|&(_, c)| !lower_in(c, class))
        .map_or(rest.len(), |(i, _)| i);
    at.saturating_add(len)
}

/// The start of the run of characters `class` accepts, lowercased, that ends
/// at `at`.
fn run_start(text: &str, at: usize, class: fn(char) -> bool) -> usize {
    let head = text.get(..at).unwrap_or_default();
    head.char_indices()
        .rev()
        .take_while(|&(_, c)| lower_in(c, class))
        .last()
        .map_or(at, |(i, _)| i)
}

/// `end` moved back over sentence marks, no further than `floor`.
fn trimmed(text: &str, floor: usize, mut end: usize) -> usize {
    while end > floor {
        match crate::text::char_before(text, end) {
            Some(c) if is_trailing(c) => end = end.saturating_sub(c.len_utf8()),
            _ => break,
        }
    }
    end
}

/// The ASCII a run of characters lowercases to, one each.
fn lower_run(run: &str) -> String {
    run.chars().filter_map(lower_one).collect()
}

/// `text` from `i` to `end`, empty where they cross.
fn between(text: &str, i: usize, end: usize) -> &str {
    text.get(i..end).unwrap_or_default()
}

/// `reply` with each memory path a person did not write in plain words.
fn scrub_paths(reply: &str, said: &str, layout: &MemoryLayout<'_>) -> String {
    let prefix = layout.prefix();
    let mut out = String::with_capacity(reply.len());
    let mut i = 0;
    while let Some(at) = find_ignoring_case(reply, prefix, i) {
        let past = at.saturating_add(chars_len(reply, at, prefix.chars().count()));
        if crate::text::char_before(reply, at).is_some_and(|c| lower_in(c, is_path_char)) {
            out.push_str(between(reply, i, past));
            i = past;
            continue;
        }
        let end = trimmed(reply, past, run_end(reply, past, is_path_char));
        let path = between(reply, at, end);
        let lower = lower_run(path);
        out.push_str(between(reply, i, at));
        out.push_str(if said.contains(lower.as_str()) {
            path
        } else {
            layout.phrase(&lower)
        });
        i = end;
    }
    out.push_str(reply.get(i..).unwrap_or_default());
    out
}

/// `reply` with each memory file named alone, whole, in plain words.
fn scrub_files(reply: String, said: &str, layout: &MemoryLayout<'_>) -> String {
    let mut out = reply;
    for &(file, phrase) in layout.files {
        if file.is_empty() || said.contains(file) {
            continue;
        }
        let mut next = String::with_capacity(out.len());
        let mut i = 0;
        while let Some(at) = find_ignoring_case(&out, file, i) {
            let past = at.saturating_add(chars_len(&out, at, file.chars().count()));
            let start = run_start(&out, at, is_file_name_char);
            let end = trimmed(&out, past, run_end(&out, past, is_file_name_char));
            let token = between(&out, start, end);
            let whole = lower_run(token) == file
                && token.chars().count() == file.chars().count()
                && crate::text::char_before(&out, start) != Some('/');
            next.push_str(between(&out, i, start));
            next.push_str(if whole { phrase } else { token });
            i = end;
        }
        next.push_str(out.get(i..).unwrap_or_default());
        out = next;
    }
    out
}

/// `reply` with each memory tool's name a person did not write in plain
/// words.
fn scrub_tool_names(reply: &str, said: &str, layout: &MemoryLayout<'_>) -> String {
    let prefix = layout.tool_prefix;
    let mut out = String::with_capacity(reply.len());
    let mut i = 0;
    while let Some(at) = find_ignoring_case(reply, prefix, i) {
        let past = at.saturating_add(chars_len(reply, at, prefix.chars().count()));
        let end = run_end(reply, past, is_word_char);
        let name = between(reply, at, end);
        let is_tool = end > past
            && crate::text::char_before(reply, at).is_none_or(|c| !lower_in(c, is_word_char));
        out.push_str(between(reply, i, at));
        out.push_str(if is_tool && !said.contains(lower_run(name).as_str()) {
            layout.tool_phrase
        } else {
            name
        });
        i = end;
    }
    out.push_str(reply.get(i..).unwrap_or_default());
    out
}

/// `reply` with its memory layout in plain words, keeping the facts around
/// it.
///
/// Each path under the memory root, memory file named alone and memory tool
/// name that the person did not write in `said` reads as its phrase.
///
/// A path inside a longer one (a repository's `…/memory/…` link), a file
/// name inside a longer one (`old-workspace.md`) or a path
/// (`docs/workspace.md`), and the person's own words are left alone. Borrowed
/// when nothing is put in plain words.
///
/// # Examples
///
/// ```
/// use figures_parser::{MemoryLayout, scrub_memory_layout};
///
/// let layout = MemoryLayout {
///     root: "/memory/",
///     tool_prefix: "memory_",
///     folders: &[],
///     files: &[("workspace.md", "the team's shared notes")],
///     root_phrase: "memory",
///     other_phrase: "the team's notes",
///     tool_phrase: "my memory",
/// };
/// let reply = "Saved with memory_write to `/memory/workspace.md`; see docs/workspace.md.";
/// assert_eq!(
///     scrub_memory_layout(reply, "", &layout),
///     "Saved with my memory to `the team's shared notes`; see docs/workspace.md.",
/// );
/// ```
#[must_use]
pub fn scrub_memory_layout<'r>(
    reply: &'r str,
    said: &str,
    layout: &MemoryLayout<'_>,
) -> Cow<'r, str> {
    let prefix = layout.prefix();
    let hit = (!prefix.is_empty() && find_ignoring_case(reply, prefix, 0).is_some())
        || (!layout.tool_prefix.is_empty()
            && find_ignoring_case(reply, layout.tool_prefix, 0).is_some())
        || layout
            .files
            .iter()
            .any(|(file, _)| !file.is_empty() && find_ignoring_case(reply, file, 0).is_some());
    if !hit {
        return Cow::Borrowed(reply);
    }
    let said = lowered(said);
    let paths = if prefix.is_empty() {
        reply.to_owned()
    } else {
        scrub_paths(reply, &said, layout)
    };
    let files = scrub_files(paths, &said, layout);
    if layout.tool_prefix.is_empty() {
        Cow::Owned(files)
    } else {
        Cow::Owned(scrub_tool_names(&files, &said, layout))
    }
}
