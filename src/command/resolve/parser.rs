use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct ConflictRegion {
    pub ours_label: Option<String>,
    pub theirs_label: Option<String>,
    pub ours: String,
    pub theirs: String,
    pub start_line: usize, // 1-based
    pub end_line: usize,   // 1-based, inclusive (line containing >>>>>>>)
    pub context_before: String,
    pub context_after: String,
}

#[derive(Debug, Clone)]
pub struct ConflictInput {
    pub path: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub working: String,
    pub regions: Vec<ConflictRegion>,
}

#[allow(dead_code)]
pub trait ConflictResolver {
    /// Returns FULL resolved file content (no narrative).
    fn resolve(&self, input: &ConflictInput) -> anyhow::Result<String>;
}

pub fn has_conflict_markers(s: &str) -> bool {
    s.contains("<<<<<<<") || s.contains("=======") || s.contains(">>>>>>>")
}

pub fn parse_conflict_regions_raw(
    working: &str,
) -> Result<Vec<(usize, usize, Option<String>, Option<String>, String, String)>> {
    // Returns vec of:
    // (start_line_1based, end_line_1based, ours_label, theirs_label, ours_text, theirs_text)
    //
    // end_line is the line containing >>>>>>> (inclusive)
    let mut regions = Vec::new();
    let mut i = 0usize;
    let lines: Vec<&str> = working.split_inclusive('\n').collect();

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("<<<<<<<") {
            let ours_label = line
                .trim_end_matches('\n')
                .strip_prefix("<<<<<<<")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let start_line = i + 1; // 1-based

            // collect ours until =======
            i += 1;
            let mut ours_buf = String::new();
            while i < lines.len() && !lines[i].starts_with("=======") {
                ours_buf.push_str(lines[i]);
                i += 1;
            }
            if i >= lines.len() || !lines[i].starts_with("=======") {
                bail!("Malformed conflict: missing ======= separator");
            }

            // skip =======
            i += 1;

            // collect theirs until >>>>>>>
            let mut theirs_buf = String::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>>") {
                theirs_buf.push_str(lines[i]);
                i += 1;
            }
            if i >= lines.len() || !lines[i].starts_with(">>>>>>>") {
                bail!("Malformed conflict: missing >>>>>>> terminator");
            }

            let theirs_label = lines[i]
                .trim_end_matches('\n')
                .strip_prefix(">>>>>>>")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let end_line = i + 1; // include >>>>>>>

            // consume >>>>>>>
            i += 1;

            regions.push((
                start_line,
                end_line,
                ours_label,
                theirs_label,
                ours_buf,
                theirs_buf,
            ));
            continue;
        }

        i += 1;
    }

    Ok(regions)
}

fn lines_slice_context(lines: &[&str], start_line_1: usize, end_line_1: usize, ctx: usize) -> (String, String) {
    // lines are split_inclusive('\n'), 1-based line indices.
    let total = lines.len();
    let start_idx = start_line_1.saturating_sub(1);
    let end_idx = end_line_1.saturating_sub(1);

    let before_start = start_idx.saturating_sub(ctx);
    let before_end = start_idx;

    let after_start = (end_idx + 1).min(total);
    let after_end = (end_idx + 1 + ctx).min(total);

    let mut before = String::new();
    for &l in &lines[before_start..before_end] {
        before.push_str(l);
    }

    let mut after = String::new();
    for &l in &lines[after_start..after_end] {
        after.push_str(l);
    }

    (before, after)
}

pub fn build_regions(working: &str, ctx_lines: usize) -> Result<Vec<ConflictRegion>> {
    let parsed = parse_conflict_regions_raw(working)?;
    let lines: Vec<&str> = working.split_inclusive('\n').collect();

    let mut regions = Vec::new();
    for (start_line, end_line, ours_label, theirs_label, ours_text, theirs_text) in parsed {
        let (before, after) = lines_slice_context(&lines, start_line, end_line, ctx_lines);
        regions.push(ConflictRegion {
            ours_label,
            theirs_label,
            ours: ours_text,
            theirs: theirs_text,
            start_line,
            end_line,
            context_before: before,
            context_after: after,
        });
    }
    Ok(regions)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptRewrite {
    Ours,
    Theirs,
    Both,
}

pub fn resolve_working_by_accept_mode(working: &str, mode: AcceptRewrite) -> Result<String> {
    // Rewrites conflict regions in the working file using the chosen mode.
    // If file is malformed, returns error.
    let mut out = String::new();
    let mut lines = working.split_inclusive('\n').peekable();

    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            // ours
            let mut ours = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with("=======") {
                    break;
                }
                ours.push_str(l);
            }

            // theirs
            let mut theirs = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with(">>>>>>>") {
                    break;
                }
                theirs.push_str(l);
            }

            match mode {
                AcceptRewrite::Ours => out.push_str(&ours),
                AcceptRewrite::Theirs => out.push_str(&theirs),
                AcceptRewrite::Both => {
                    out.push_str(&ours);
                    out.push_str(&theirs);
                }
            }

            continue;
        }

        out.push_str(line);
    }

    Ok(out)
}
