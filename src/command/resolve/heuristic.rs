fn strip_all_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

pub fn choose_region_heuristic(ours: &str, theirs: &str) -> Option<String> {
    // Returns Some(resolution) when we are confident to auto-resolve.
    // Otherwise None => needs LLM.
    if ours == theirs {
        return Some(ours.to_string());
    }

    let o = ours.trim();
    let t = theirs.trim();

    if o.is_empty() && !t.is_empty() {
        return Some(theirs.to_string());
    }
    if t.is_empty() && !o.is_empty() {
        return Some(ours.to_string());
    }

    // whitespace-only difference
    if strip_all_whitespace(ours) == strip_all_whitespace(theirs) {
        // Prefer ours deterministically
        return Some(ours.to_string());
    }

    None
}

pub fn resolve_working_by_heuristics(working: &str) -> (String, bool) {
    // Returns (resolved, fully_resolved_without_llm)
    // If any region cannot be auto-resolved, we return (working, false).
    let mut out = String::new();
    let mut lines = working.split_inclusive('\n').peekable();

    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            // collect ours
            let mut ours = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with("=======") {
                    break;
                }
                ours.push_str(l);
            }
            // collect theirs
            let mut theirs = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with(">>>>>>>") {
                    break;
                }
                theirs.push_str(l);
            }

            match choose_region_heuristic(&ours, &theirs) {
                Some(chosen) => out.push_str(&chosen),
                None => {
                    // Needs LLM
                    return (working.to_string(), false);
                }
            }
            continue;
        }

        out.push_str(line);
    }

    (out, true)
}
