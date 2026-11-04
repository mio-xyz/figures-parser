use figures_parser::{SilentSignal, memory_claim, silent_signal, speaker_prefix};

#[test]
fn reads_a_speaker_prefix_on_the_first_line_only() {
    assert_eq!(speaker_prefix("[Mio]: hello"), Some("[Mio]: "));
    assert_eq!(
        speaker_prefix("  \n[Ana Lopez]:\tsee below"),
        Some("[Ana Lopez]:\t")
    );
    assert_eq!(speaker_prefix("[a[b]: x"), Some("[a[b]: "));
    assert_eq!(speaker_prefix("[link](https://x)"), None);
    assert_eq!(
        speaker_prefix("See the doc.\n[ref]: https://example.com"),
        None
    );
    assert_eq!(speaker_prefix("[]: x"), None);
    assert_eq!(speaker_prefix("[Mio]:hello"), None);
    assert_eq!(speaker_prefix("[Mi\no]: x"), None);
    // The name holds at most 80 UTF-16 units; an emoji takes two.
    let long = "a".repeat(80);
    assert!(speaker_prefix(&format!("[{long}]: x")).is_some());
    assert!(speaker_prefix(&format!("[{long}b]: x")).is_none());
    assert!(speaker_prefix(&format!("[{}😀]: x", "a".repeat(79))).is_none());
}

#[test]
fn reads_a_memory_save_claim_and_no_write_elsewhere() {
    for (reply, claim) in [
        ("Done, I'll remember that for next time.", "I'll remember"),
        (
            "I've saved that to your preferences.",
            "saved that to your preferences",
        ),
        ("I've stored that in my memory.", "stored that in my memory"),
        (
            "I've updated that in your profile.",
            "updated that in your profile",
        ),
        ("i will remember", "i will remember"),
        ("Committed it to memory.", "Committed it to memory"),
        ("I have noted it.", "I have noted"),
    ] {
        assert_eq!(memory_claim(reply), Some(claim), "{reply}");
    }
    for reply in [
        "Keep in mind the deadline is Friday.",
        "I've saved it to Notion.",
        "I've added that to the doc.",
        "I'll remind you tomorrow.",
        "AI'll remember",
    ] {
        assert_eq!(memory_claim(reply), None, "{reply}");
    }
}

// The TypeScript check reads a straight apostrophe only; a reply written
// with a curly one makes the same claim.
#[test]
fn reads_a_memory_claim_with_a_curly_apostrophe() {
    assert_eq!(memory_claim("I’ll remember that."), Some("I’ll remember"));
    assert_eq!(memory_claim("I’ve noted it."), Some("I’ve noted"));
}

#[test]
fn reads_the_silent_marker_whole_or_leaked() {
    let whole = |reason| {
        Some(SilentSignal {
            reason,
            leaked: false,
        })
    };
    let leaked = |reason| {
        Some(SilentSignal {
            reason,
            leaked: true,
        })
    };
    assert_eq!(silent_signal("silent: nothing_new"), whole("nothing_new"));
    assert_eq!(silent_signal("  silent:no_signal \n"), whole("no_signal"));
    assert_eq!(silent_signal("[CURRENT MESSAGE] silent: x"), whole("x"));
    assert_eq!(silent_signal("silent:\nno_signal"), whole("no_signal"));
    assert_eq!(
        silent_signal("silent: nothing_new\nBut also here is a summary."),
        leaked("nothing_new")
    );
    assert_eq!(silent_signal("Summary.\r\nsilent: a \r\nmore"), leaked("a"));
    assert_eq!(silent_signal("Summary.\nsilent:\nfoo\nbar"), leaked("foo"));
    assert_eq!(silent_signal("Hello"), None);
    assert_eq!(silent_signal("silent: two words"), None);
    assert_eq!(silent_signal("Summary.\nsilent: two words"), None);
    assert_eq!(silent_signal("Silent: x"), None);
    assert_eq!(silent_signal("note: silent: x"), None);
    assert_eq!(silent_signal("silent:"), None);
}
