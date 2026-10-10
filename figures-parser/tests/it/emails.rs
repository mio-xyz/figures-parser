use figures_parser::{emails, ungrounded_emails};

fn texts(s: &str) -> Vec<&str> {
    emails(s).into_iter().map(|e| e.text).collect()
}

#[test]
fn finds_addresses_in_prose_markdown_and_slack_links() {
    assert_eq!(
        texts(
            "Write to ana@acme.io. Or [Ben](mailto:ben.k@acme.io), <mailto:cy+ops@acme.co.uk|Cy>!"
        ),
        ["ana@acme.io", "ben.k@acme.io", "cy+ops@acme.co.uk"]
    );
    assert_eq!(
        texts("no address @here, nor ana@localhost"),
        Vec::<&str>::new()
    );
}

#[test]
fn an_address_slices_the_input_at_its_span() {
    let text = "😀 mail **ana@acme.io**.";
    for email in emails(text) {
        assert_eq!(text.get(email.span), Some(email.text));
    }
}

#[test]
fn flags_an_address_no_result_or_message_gave() {
    let corpus = r#"{"owner": "ana@acme.io"}"#;
    assert_eq!(
        ungrounded_emails("Ask ana@acme.io or ben@acme.io.", corpus, ""),
        ["ben@acme.io"]
    );
}

#[test]
fn grounds_an_address_written_in_another_case_and_reports_each_once() {
    assert_eq!(
        ungrounded_emails("Ana@Acme.IO and ana@acme.io", "owner: ana@acme.io", ""),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_emails("bob@acme.io, Bob@acme.io", "", ""),
        ["bob@acme.io"]
    );
}

#[test]
fn grounds_an_address_the_prompt_carried_except_on_an_example_domain() {
    let prompt = "The asker is ana@acme.io. Format: [Name](mailto:jane@example.com).";
    assert_eq!(
        ungrounded_emails("Sent to ana@acme.io.", "", prompt),
        Vec::<String>::new()
    );
    assert_eq!(
        ungrounded_emails("Reach jane@example.com.", "", prompt),
        ["jane@example.com"]
    );
    assert_eq!(
        ungrounded_emails("Reach jane@example.com.", "from: jane@example.com", prompt),
        Vec::<String>::new()
    );
}

#[test]
fn shares_the_link_kinds_example_domains() {
    assert_eq!(
        ungrounded_emails("Reach jane@www.example.com.", "", "jane@www.example.com"),
        ["jane@www.example.com"]
    );
    assert_eq!(
        ungrounded_emails("Reach jane@docs.example.com.", "", "jane@docs.example.com"),
        Vec::<String>::new()
    );
}
