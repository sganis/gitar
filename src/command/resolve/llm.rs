use anyhow::{bail, Context, Result};
use std::io::{self, Write};

use crate::client::LlmClient;

use super::diff_preview;
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

fn conflict_block_for_preview(region: &ConflictRegion) -> String {
    // For diff preview only; labels are just hints.
    let mut out = String::new();

    out.push_str("<<<<<<< ");
    if let Some(l) = &region.ours_label {
        out.push_str(l);
    } else {
        out.push_str("OURS");
    }
    out.push('\n');

    out.push_str(&region.ours);
    if !region.ours.ends_with('\n') && !region.ours.is_empty() {
        out.push('\n');
    }

    out.push_str("=======\n");

    out.push_str(&region.theirs);
    if !region.theirs.ends_with('\n') && !region.theirs.is_empty() {
        out.push('\n');
    }

    out.push_str(">>>>>>> ");
    if let Some(l) = &region.theirs_label {
        out.push_str(l);
    } else {
        out.push_str("THEIRS");
    }
    out.push('\n');

    out
}

fn prompt_region_action(region_no_1: usize, attempt_no_1: usize) -> Result<char> {
    eprint!(
        "  Region {} (attempt {}): accept [y], retry [r], fallback to full-file [f], quit [q] > ",
        region_no_1, attempt_no_1
    );
    io::stderr().flush().ok();

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let s = line.trim().to_lowercase();
    Ok(s.chars().next().unwrap_or('q'))
}

async fn resolve_region_with_llm(
    client: &LlmClient,
    stream: bool,
    input: &ConflictInput,
    region_idx_0: usize,
    region: &ConflictRegion,
    prev_attempt: Option<&str>,
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

    if let Some(prev) = prev_attempt {
        user.push_str("=== PREVIOUS ATTEMPT (was rejected) ===\n");
        user.push_str(prev);
        if !prev.ends_with('\n') {
            user.push('\n');
        }
        user.push('\n');
        user.push_str("Try a better resolution.\n\n");
    }

    let raw = client.chat(system, &user, stream).await?;
    let cleaned = strip_markdown_fences(&raw);

    if has_conflict_markers(&cleaned) {
        bail!("Region LLM output contains conflict markers");
    }

    Ok(cleaned)
}

pub async fn resolve_working_per_region_with_fallback(
    client: &LlmClient,
    stream: bool,
    input: &ConflictInput,
) -> Result<String> {
    let working = &input.working;

    let mut out = String::new();
    let mut lines = working.split_inclusive('\n').peekable();
    let mut region_idx = 0usize;

    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            let mut ours = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with("=======") {
                    break;
                }
                ours.push_str(l);
            }

            let mut theirs = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with(">>>>>>>") {
                    break;
                }
                theirs.push_str(l);
            }

            let region = input
                .regions
                .get(region_idx)
                .with_context(|| format!("Region index {} out of bounds", region_idx + 1))?;

            if region.ours != ours || region.theirs != theirs {
                let fallback = resolve_with_llm_full_file(client, stream, input).await?;
                return Ok(fallback);
            }

            if let Some(chosen) = heuristic::choose_region_heuristic(&ours, &theirs) {
                out.push_str(&chosen);
            } else {
                let resolved_block = match resolve_region_with_llm(client, stream, input, region_idx, region, None).await {
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
        let fallback = resolve_with_llm_full_file(client, stream, input).await?;
        return Ok(fallback);
    }

    Ok(out)
}

pub async fn resolve_working_per_region_interactive(
    client: &LlmClient,
    stream: bool,
    input: &ConflictInput,
) -> Result<String> {
    let working = &input.working;

    let mut out = String::new();
    let mut lines = working.split_inclusive('\n').peekable();
    let mut region_idx = 0usize;

    while let Some(line) = lines.next() {
        if line.starts_with("<<<<<<<") {
            let mut ours = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with("=======") {
                    break;
                }
                ours.push_str(l);
            }

            let mut theirs = String::new();
            while let Some(l) = lines.next() {
                if l.starts_with(">>>>>>>") {
                    break;
                }
                theirs.push_str(l);
            }

            let region = input
                .regions
                .get(region_idx)
                .with_context(|| format!("Region index {} out of bounds", region_idx + 1))?;

            // If our scan doesn't match parsed model, don't play games: fallback.
            if region.ours != ours || region.theirs != theirs {
                eprintln!("  Region parsing mismatch; falling back to full-file LLM.");
                return resolve_with_llm_full_file(client, stream, input).await;
            }

            if let Some(chosen) = heuristic::choose_region_heuristic(&ours, &theirs) {
                eprintln!("  Region {}: auto-resolved by heuristic.", region_idx + 1);
                out.push_str(&chosen);
                region_idx += 1;
                continue;
            }

            eprintln!("  Region {}: needs LLM.", region_idx + 1);

            let mut prev: Option<String> = None;
            let max_retries = 3usize;
            let mut attempt = 0usize;

            loop {
                attempt += 1;

                let resolved = resolve_region_with_llm(
                    client,
                    stream,
                    input,
                    region_idx,
                    region,
                    prev.as_deref(),
                )
                .await
                .with_context(|| format!("Region {} LLM failed", region_idx + 1))?;

                if has_conflict_markers(&resolved) {
                    bail!("Region {} output contains conflict markers", region_idx + 1);
                }

                // Region preview diff
                let before_block = conflict_block_for_preview(region);
                eprintln!("  Region {} proposed diff:", region_idx + 1);
                diff_preview::show_preview_diff_virtual(
                    &format!("{}-region-{}", input.path, region_idx + 1),
                    &before_block,
                    &resolved,
                )?;

                let action = prompt_region_action(region_idx + 1, attempt)?;
                match action {
                    'y' => {
                        out.push_str(&resolved);
                        break;
                    }
                    'r' => {
                        if attempt >= max_retries {
                            eprintln!("  Region {}: max retries reached; falling back to full-file LLM.", region_idx + 1);
                            return resolve_with_llm_full_file(client, stream, input).await;
                        }
                        prev = Some(resolved);
                        continue;
                    }
                    'f' => {
                        eprintln!("  Falling back to full-file LLM.");
                        return resolve_with_llm_full_file(client, stream, input).await;
                    }
                    'q' => {
                        bail!("Aborted by user");
                    }
                    _ => {
                        eprintln!("  Unknown choice. Use y/r/f/q.");
                        prev = Some(resolved);
                        attempt -= 1; // don't count garbage input
                        continue;
                    }
                }
            }

            region_idx += 1;
            continue;
        }

        out.push_str(line);
    }

    if has_conflict_markers(&out) {
        eprintln!("  Safety fallback: markers remain; using full-file LLM.");
        return resolve_with_llm_full_file(client, stream, input).await;
    }

    Ok(out)
}
