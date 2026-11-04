use figures_parser::{HOLDS_PER_PAGE, PageHolds, ungrounded_on_page};

const PAGE: &str = "Sprint 42: onboarding shipped, activation up 12% on 1,840 sign-ups. Unsubscribe at https://quesly.example/u";

#[test]
fn names_the_figures_and_ratios_of_a_page_nothing_printed_never_its_links() {
    let mut found = ungrounded_on_page(PAGE, "", "", false);
    found.sort();
    assert_eq!(found, ["1,840", "12%"]);
}

#[test]
fn passes_what_a_result_printed() {
    let corpus = "signups 1840\nactivation_change 0.12";
    assert_eq!(
        ungrounded_on_page(PAGE, corpus, "", false),
        Vec::<String>::new()
    );
}

#[test]
fn names_an_id_no_result_printed() {
    let page = "Refunded prod_Zq81LmN07tYwPa today.";
    assert_eq!(
        ungrounded_on_page(page, "", "", false),
        ["prod_Zq81LmN07tYwPa"]
    );
}

#[test]
fn holds_back_a_page_for_a_claim_it_was_not_told_about() {
    let mut holds = PageHolds::default();
    let none: &[&str] = &[];
    assert!(!holds.hold("mockup.png", none));
    assert!(holds.hold("mockup.png", &["12%"]));
    assert!(!holds.hold("mockup.png", &["12%"]));
    assert!(holds.hold("mockup.png", &["12%", "40%"]));
    assert!(holds.hold("other.png", &["12%"]));
}

#[test]
fn attaches_a_page_once_it_was_held_back_three_times() {
    let mut holds = PageHolds::default();
    let held: Vec<bool> = (1..=HOLDS_PER_PAGE + 1)
        .map(|n| holds.hold("p.png", &[format!("claim{n}")]))
        .collect();
    assert_eq!(held, [true, true, true, false]);
}
