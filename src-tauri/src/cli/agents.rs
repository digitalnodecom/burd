//! Writes Burd's managed hint block into a project's AGENTS.md so AI coding
//! agents working in the directory know to use Burd for local services instead
//! of Docker / ad-hoc servers.
//!
//! The block is delimited by stable markers (see `agent_guidance::BLOCK_BEGIN`
//! / `BLOCK_END`). Content between the markers is owned by Burd and refreshed in
//! place; everything else in the file is the user's and never touched. All
//! writes are prompted.

use std::fs;
use std::path::Path;

use crate::agent_guidance::{project_agents_block, ProjectFacts, BLOCK_BEGIN, BLOCK_END};
use crate::cli::confirm;

const AGENTS_FILE: &str = "AGENTS.md";

/// Offer to add or refresh Burd's managed block in `<project_dir>/AGENTS.md`.
///
/// Behaviour:
/// - no AGENTS.md            → prompt to create it with the block;
/// - AGENTS.md with markers  → refresh the block in place, silently if the
///   content is already identical (no nag on repeat `burd init`);
/// - AGENTS.md without markers → prompt to append the block (never rewrites the
///   user's existing content).
///
/// Never fatal: any IO error is reported as a warning and swallowed so it can't
/// block instance setup.
pub fn offer_agents_hint(project_dir: &Path, facts: &ProjectFacts) {
    let path = project_dir.join(AGENTS_FILE);
    let block = project_agents_block(facts);

    let existing = match fs::read_to_string(&path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!("Warning: couldn't read {}: {}", AGENTS_FILE, e);
            return;
        }
    };

    match existing {
        // Fresh file.
        None => {
            if !confirm(
                "Add AGENTS.md so AI agents use Burd (not Docker) for this project?",
                true,
            ) {
                return;
            }
            if let Err(e) = fs::write(&path, &block) {
                eprintln!("Warning: failed to write {}: {}", AGENTS_FILE, e);
            } else {
                println!("✓ Wrote {} (Burd guidance for AI agents)", AGENTS_FILE);
            }
        }

        // Existing file already contains our managed block.
        Some(content) if content.contains(BLOCK_BEGIN) && content.contains(BLOCK_END) => {
            let updated = replace_block(&content, &block);
            if updated == content {
                // Already up to date — stay quiet.
                return;
            }
            if let Err(e) = fs::write(&path, &updated) {
                eprintln!("Warning: failed to update {}: {}", AGENTS_FILE, e);
            } else {
                println!("✓ Refreshed Burd guidance in {}", AGENTS_FILE);
            }
        }

        // Existing file, no managed block — append (prompted).
        Some(content) => {
            if !confirm(
                "Append Burd guidance for AI agents to your existing AGENTS.md?",
                true,
            ) {
                return;
            }
            let sep = if content.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            let updated = format!("{}{}{}", content, sep, block);
            if let Err(e) = fs::write(&path, &updated) {
                eprintln!("Warning: failed to update {}: {}", AGENTS_FILE, e);
            } else {
                println!("✓ Added Burd guidance to {}", AGENTS_FILE);
            }
        }
    }
}

/// Replace the text between the managed markers (inclusive) with `block`.
/// Assumes both markers are present; falls back to returning `content`
/// unchanged if they're malformed (end before begin).
fn replace_block(content: &str, block: &str) -> String {
    let (Some(start), Some(end_marker)) = (content.find(BLOCK_BEGIN), content.find(BLOCK_END))
    else {
        return content.to_string();
    };
    if end_marker < start {
        return content.to_string();
    }
    let end = end_marker + BLOCK_END.len();
    // `block` carries a trailing newline; drop the one already after the old
    // end-marker if present so we don't accumulate blank lines on refresh.
    let mut tail = &content[end..];
    tail = tail.strip_prefix('\n').unwrap_or(tail);
    format!("{}{}{}", &content[..start], block, tail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> ProjectFacts {
        ProjectFacts {
            url: Some("https://myapp.test".to_string()),
            tld: "test".to_string(),
            parked: false,
        }
    }

    #[test]
    fn replace_is_idempotent() {
        let block = project_agents_block(&facts());
        let seeded = format!("# My project\n\nSome notes.\n\n{}", block);
        let once = replace_block(&seeded, &block);
        let twice = replace_block(&once, &block);
        assert_eq!(once, twice);
        assert!(once.contains("# My project"));
        assert_eq!(once.matches(BLOCK_BEGIN).count(), 1);
    }

    #[test]
    fn replace_preserves_surrounding_content() {
        let block = project_agents_block(&facts());
        let seeded = format!("HEADER\n{}\nFOOTER\n", block);
        let out = replace_block(&seeded, &block);
        assert!(out.starts_with("HEADER\n"));
        assert!(out.trim_end().ends_with("FOOTER"));
    }
}
