use figures_parser::{MispairedFigure, mispaired_per_unit};

/// A script's rows with a lifetime window and yesterday's, the per-unit
/// amount printed for the lifetime one only.
const PRINTED: &str = "run_code: exit 0
[ACTIVE] Rework | created: 2026-10-29 | LT: $91.65 / 2 leads / CPL $45.83 / CTR 1.875902 | Y: $74.74 / 2 leads
[ACTIVE] Main | LT: $1,679.41 / 24 leads / CPL $69.98 | Y: $280.03 / 5 leads";

const fn none() -> Vec<MispairedFigure> {
    Vec::new()
}

#[test]
fn names_a_per_unit_amount_stated_beside_another_windows_total_and_count() {
    assert_eq!(
        mispaired_per_unit(
            "*Rework* launched yesterday. $74.74 spend, 2 leads, $45.83 CPL, CTR 1.88%.",
            PRINTED
        ),
        [MispairedFigure {
            per_unit: "$45.83".to_owned(),
            stated: "$74.74 / 2 = $37.37".to_owned(),
            printed: "$91.65 / 2".to_owned(),
        }]
    );
}

#[test]
fn leaves_a_per_unit_amount_its_own_clauses_total_and_count_give() {
    for reply in [
        "$74.74 spend, 2 leads, $37.37 CPL.",
        "$91.65 lifetime, 2 leads, $45.83 CPL.",
        "$1,679 lifetime since Oct 21, 24 leads, $69.98 blended CPL.",
        "$1,680 lifetime, 24 leads, $69.98 CPL.",
    ] {
        assert_eq!(mispaired_per_unit(reply, PRINTED), none(), "{reply}");
    }
}

#[test]
fn reads_each_sentence_on_its_own_a_stop_before_a_lower_case_word_included() {
    assert_eq!(
        mispaired_per_unit(
            "$74.74 yesterday, 2 leads. $91.65 lifetime, 2 leads, $45.83 CPL.",
            PRINTED
        ),
        none()
    );
    assert_eq!(
        mispaired_per_unit("$74.74 yesterday, 2 leads. then $45.83 CPL.", PRINTED),
        none()
    );
    assert_eq!(
        mispaired_per_unit("$74.74 spend\n2 leads, $45.83 CPL", PRINTED),
        none()
    );
}

#[test]
fn leaves_a_per_unit_amount_no_printed_line_shows_as_a_quotient() {
    let reply = "$74.74 spend, 2 leads, $45.83 CPL.";
    assert_eq!(
        mispaired_per_unit(reply, "Spend: $91.65\nLeads: 2\nCPL: $45.83"),
        none()
    );
    assert_eq!(mispaired_per_unit(reply, ""), none());
}

#[test]
fn leaves_a_round_per_unit_amount_which_any_amount_over_some_count_may_land_on() {
    assert_eq!(
        mispaired_per_unit("$300 spend, 2 leads, $100 CPL.", "LT: $200.00 / 2 leads"),
        none()
    );
}

#[test]
fn reads_two_per_unit_amounts_side_by_side_as_two_not_as_a_total_and_its_quotient() {
    let rows = "| A | $253.17 | 5 leads | $50.63 |\n| B | $108.52 | 3 leads | $36.17 |";
    assert_eq!(
        mispaired_per_unit(
            "Best so far: A ($50.63 CPL, 5 leads) and B ($36.17 CPL, 3 leads).",
            rows
        ),
        none()
    );
}

#[test]
fn reads_no_candidate_in_a_clause_without_two_amounts_and_a_count() {
    for reply in [
        "Spend held at $74.74 yesterday.",
        "$74.74 spend and $45.83 CPL.",
        "74.74 spend, 2 leads, 45.83 CPL.",
    ] {
        assert_eq!(mispaired_per_unit(reply, PRINTED), none(), "{reply}");
    }
}

#[test]
fn leaves_figures_inside_a_link_or_a_ratio_or_with_a_scale_out() {
    assert_eq!(
        mispaired_per_unit("$74.74 spend, 2 leads, 45.83% CTR.", PRINTED),
        none()
    );
    assert_eq!(
        mispaired_per_unit(
            "$74.74 spend, 2 leads, see https://ads.acme.io/cpl/$45.83 now.",
            PRINTED
        ),
        none()
    );
}

#[test]
fn prints_the_amounts_grouped_and_rounded_as_english_does() {
    let printed = "Q: $2,500.10 / 3 orders / AOV $833.37";
    assert_eq!(
        mispaired_per_unit("$1,250.5 revenue, 3 orders, $833.37 AOV.", printed),
        [MispairedFigure {
            per_unit: "$833.37".to_owned(),
            stated: "$1,250.5 / 3 = $416.83".to_owned(),
            printed: "$2,500.10 / 3".to_owned(),
        }]
    );
}
