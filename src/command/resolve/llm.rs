use anyhow::{bail, Context, Result};

use crate::client::LlmClient;

use super::heuristic;
use super::parser::{has_conflict_markers, ConflictInput, ConflictRegion};

pub async fn resolve_with_llm_full_file(client: &LlmClient, stream: bool, input: &ConflictInput) -> Result<String> {
    let system = "You are a merge conflict resolver.\n\
Return ONLY the full resolved file content.\n\
Rules:\n\
- Do not include any explanation.\n\
- Do not include markdown fences.\n\
- Preserve both intents when possible.\n\
- Ensure conflict markers are removed.\n";

    let mut user = String::new();
    user.push_str("Resolve merge conflicts for this file.\n\n");
    user.push_str(&format!("PATH: {}\n\n", input.path));

    if let Some(base) = &input.base {
        user.push_str("=== BASE (stage 1) ===\n");
        user.push_str(base);
        if !base.ends_with('\n') {
            user.push('\n');
        }
        user.push('\n');
    }
    if let Some(ours) = &input.ours {
        user.push_str("=== OURS (stage 2) ===\n");
        user.push_str(ours);
        if !ours.ends_with('\n') {
            user.push('\n');
        }
        user.push('\n');
    }
    if let Some(theirs) = &input.theirs {
        user.push_str("=== THEIRS (stage 3) ===\n");
        user.push_str(theirs);
        if !theirs.ends_with('\n') {
            user.push('\n');
        }
        user.push('\n');
    }

    user.push_str("=== WORKING FILE (with conflict markers) ===\n");
    user.push_str(&input.working);
    if !input.working.ends_with('\n') {
        user.push('\n');
    }
    user.push('\n');

    user.push_str("=== CONFLICT REGIONS (for reference) ===\n");
    for (idx, r) in input.regions.iter().enumerate() {
        user.push_str(&format!("REGION {}: lines {}-{}\n", idx + 1, r.start_line, r.end_line));
        if let Some(l) = &r.ours_label {
            user.push_str(&format!("OURS LABEL: {}\n", l));
        }
        if let Some(l) = &r.theirs_label {
            user.push_str(&format!("THEIRS LABEL: {}\n", l));
        }
        user.push_str("--- CONTEXT BEFORE ---\n");
        user.push_str(&r.context_before);
        if !r.context_before.ends_with('\n') && !r.context_before.is_empty() {
            user.push('\n');
        }
        user.push_str("--- OURS ---\n");
        user.push_str(&r.ours);
        if !r.ours.ends_with('\n') && !r.ours.is_empty() {
            user.push('\n');
        }
        user.push_str("--- THEIRS ---\n");
        user.push_str(&r.theirs);
        if !r.theirs.ends_with('\n') && !r.theirs.is_empty() {
            user.push('\n');
        }
        user.push_str("--- CONTEXT AFTER ---\n");
        user.push_str(&r.context_after);
        if !r.context_after.ends_with('\n') && !r.context_after.is_empty() {
            user.push('\n');
        }
        user.push('\n');
    }

    client.chat(system, &user, stream).await
}

fn strip_markdown_fences(s: &str) -> String {
    // Defensive: region-mode asks for "no fences", but models sometimes add them.
    let mut out = String::new();
    for line in s.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn take_last_n_lines(s: &str, n: usize) -> String {
    let mut lines: Vec<&str> = s.lines().collect();
    if lines.len() > n {
        lines = lines[lines.len() - n..].to_vec();
    }
    let mut out = String::new();
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn take_first_n_lines(s: &str, n: usize) -> String {
    let mut out = String::new();
    for (i, l) in s.lines().enumerate() {
        if i >= n {
            break;
        }
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn extract_anchor_snippet(blob: &str, before_ctx: &str, after_ctx: &str) -> Option<String> {
    // Best-effort: locate a snippet in `blob` using anchors derived from context.
    // We use a small tail of BEFORE and a small head of AFTER as anchors.
    let before_anchor = take_last_n_lines(before_ctx, 3);
    let after_anchor = take_first_n_lines(after_ctx, 3);

    let b = before_anchor.trim_end();
    let a = after_anchor.trim_end();

    if b.is_empty() || a.is_empty() {
        return None;
    }

    let start = blob.find(b)?;
    let start_pos = start + b.len();
    let rest = &blob[start_pos..];
    let end_rel = rest.find(a)?;
    let mid = &rest[..end_rel];

    // Keep it bounded (avoid surprises)
    let mut out = String::new();
    out.push_str(b);
    if !b.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(mid);
    if !mid.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(a);
    if !a.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

async fn resolve_region_with_llm(
    client: &LlmClient,
    stream: bool,
    input: &ConflictInput,
    region_idx_0: usize,
    region: &ConflictRegion,
) -> Result<String> {
    let system = "You are a merge conflict resolver.\n\
Return ONLY the resolved block for the conflict region.\n\
Rules:\n\
- Do not include any explanation.\n\
- Do not include markdown fences.\n\
- Output must be valid code/text for insertion.\n\
- Do not include conflict markers.\n";

    let mut user = String::new();
    user.push_str("Resolve this single merge conflict region.\n\n");
    user.push_str(&format!("PATH: {}\n", input.path));
    user.push_str(&format!("REGION: {}\n\n", region_idx_0 + 1));

    // Best-effort base snippet (cheap, optional).
    if let Some(base) = &input.base {
        if let Some(sn) = extract_anchor_snippet(base, &region.context_before, &region.context_after) {
            user.push_str("=== BASE SNIPPET (best-effort) ===\n");
            user.push_str(&sn);
            if !sn.ends_with('\n') {
                user.push('\n');
            }
            user.push('\n');
        }
    }

    user.push_str("=== CONTEXT BEFORE (keep in mind) ===\n");
    user.push_str(&region.context_before);
    if !region.context_before.ends_with('\n') && !region.context_before.is_empty() {
        user.push('\n');
    }
    user.push('\n');

    user.push_str("=== OURS ===\n");
    user.push_str(&region.ours);
    if !region.ours.ends_with('\n') && !region.ours.is_empty() {
        user.push('\n');
    }
    user.push('\n');

    user.push_str("=== THEIRS ===\n");
    user.push_str(&region.theirs);
    if !region.theirs.ends_with('\n') && !region.theirs.is_empty() {
        user.push('\n');
    }
    user.push('\n');

    user.push_str("=== CONTEXT AFTER (keep in mind) ===\n");
    user.push_str(&region.context_after);
    if !region.context_after.ends_with('\n') && !region.context_after.is_empty() {
        user.push('\n');
    }
    user.push('\n');

    let raw = client.chat(system, &user, stream).await?;
    let cleaned = strip_markdown_fences(&raw);

    if has_conflict_markers(&cleaned) {
        bail!("Region LLM output contains conflict markers");
    }

    // Also reject if it looks like it returned headings / narrative.
    // Keep it conservative: allow anything except obvious markdown code fences (already stripped).
    Ok(cleaned)
}

pub async fn resolve_working_per_region_with_fallback(
    client: &LlmClient,
    stream: bool,
    input: &ConflictInput,
) -> Result<String> {
    // Walk the working file and replace each conflict region.
    // For each region:
    // - Try heuristic on (ours, theirs).
    // - Else call per-region LLM.
    //
    // If anything fails, fallback to full-file LLM.
    let working = &input.working;

    let mut out = String::new();
    let mut lines = working.split_inclusive('\n').peekable();
    let mut region_idx = 0usize;

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

            // Use pre-parsed region for context (must stay aligned).
            let region = input
                .regions
                .get(region_idx)
                .with_context(|| format!("Region index {} out of bounds", region_idx + 1))?;

            // Sanity: match parsed blocks with scanned blocks (cheap guard).
            // If mismatch, bail to full-file fallback.
            if region.ours != ours || region.theirs != theirs {
                let fallback = resolve_with_llm_full_file(client, stream, input).await?;
                return Ok(fallback);
            }

            if let Some(chosen) = heuristic::choose_region_heuristic(&ours, &theirs) {
                out.push_str(&chosen);
            } else {
                let resolved_block = match resolve_region_with_llm(client, stream, input, region_idx, region).await {
                    Ok(b) => b,
                    Err(_) => {
                        let fallback = resolve_with_llm_full_file(client, stream, input).await?;
                        return Ok(fallback);
                    }
                };

                if has_conflict_markers(&resolved_block) {
                    let fallback = resolve_with_llm_full_file(client, stream, input).await?;
                    return Ok(fallback);
                }

                out.push_str(&resolved_block);
            }

            region_idx += 1;
            continue;
        }

        out.push_str(line);
    }

    if has_conflict_markers(&out) {
        // Safety: should never happen, but fallback if it does.
        let fallback = resolve_with_llm_full_file(client, stream, input).await?;
        return Ok(fallback);
    }

    Ok(out)
}
