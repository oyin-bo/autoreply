use proptest::prelude::*;

// Local copy of the highlighting merge logic (keeps tests self-contained).
fn highlight_merge(text: &str, query: &str) -> String {
    if query.is_empty() {
        return text.to_string();
    }

    let lower = text.to_lowercase();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for term in query.split_whitespace() {
        if term.is_empty() {
            continue;
        }
        let term_l = term.to_lowercase();
        let mut idx = 0usize;
        while let Some(pos) = lower[idx..].find(&term_l) {
            let abs = idx + pos;
            ranges.push((abs, abs + term.len()));
            idx = abs + term.len();
        }
    }

    if ranges.is_empty() {
        return text.to_string();
    }

    ranges.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in ranges {
        if let Some(last) = merged.last_mut() {
            if s <= last.1 {
                if e > last.1 {
                    last.1 = e;
                }
            } else {
                if s == last.1 + 1 && text.as_bytes()[last.1] == b'\n' {
                    if e > last.1 {
                        last.1 = e;
                    }
                } else {
                    merged.push((s, e));
                }
            }
        } else {
            merged.push((s, e));
        }
    }

    let mut res = String::new();
    let mut last_idx = 0usize;
    for (s, e) in merged {
        if last_idx < s {
            res.push_str(&text[last_idx..s]);
        }
        res.push_str("**");
        res.push_str(&text[s..e]);
        res.push_str("**");
        last_idx = e;
    }
    if last_idx < text.len() {
        res.push_str(&text[last_idx..]);
    }

    res
}

// Property test: single newline between matches should allow merge, double newline should not
proptest! {
    #[test]
    fn highlight_newline_merge_behaviour(a in proptest::char::range('a','z'), b in proptest::char::range('a','z')) {
        let a_s = a.to_string();
        let b_s = b.to_string();

        let text_single = format!("{}\n{}", a_s, b_s);
        let md_single = highlight_merge(&text_single, &format!("{} {}", a_s, b_s));
        let expected_single = format!("**{}\n{}**", a_s, b_s);
        prop_assert!(md_single.contains(&expected_single));

        let text_double = format!("{}\n\n{}", a_s, b_s);
        let md_double = highlight_merge(&text_double, &format!("{} {}", a_s, b_s));
        let expected_double = format!("**{}\n\n{}**", a_s, b_s);
        prop_assert!(!md_double.contains(&expected_double));
    }
}
