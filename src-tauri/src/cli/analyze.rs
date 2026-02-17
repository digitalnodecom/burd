//! Analyze CLI command
//!
//! Analyzes PHP projects to detect type, configuration, and suggest improvements.

use crate::analyzer::{analyze_with_burd_config, IssueSeverity, ProjectInfo, ProjectType};
use crate::config::ConfigStore;
use crate::pvm;
use std::env;

/// Run the analyze command
///
/// Analyzes the current directory to detect project type,
/// parse configuration, and check against Burd services.
pub fn run_analyze() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let info = analyze_with_burd_config(&current_dir, &config)?;

    print_analysis(&info);

    Ok(())
}

/// Print the project analysis results
fn print_analysis(info: &ProjectInfo) {
    println!();

    // Header
    println!("Project Analysis: {}", info.name);
    println!("{}", "=".repeat(40));

    // Basic info
    println!("Type: {}", info.project_type.display_name());
    println!("Path: {}", info.path.display());
    println!(
        "Document Root: {}",
        info.document_root
            .strip_prefix(&info.path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| info.document_root.display().to_string())
    );

    if let Some(instance_ver) = &info.instance_php_version {
        println!("PHP Version: {} (Instance)", instance_ver);
    }

    // Show PHP CLI with source detection
    if let Some(current) = pvm::get_current_php() {
        println!("PHP CLI:     {} ({})", current.version, current.source);
        if current.source != "Burd" {
            println!("  [!] PHP CLI is provided by {} — not Burd", current.source);
            println!("      Path: {}", current.path);
            if let Some(burd_php) = pvm::get_burd_php() {
                println!(
                    "      -> Burd has PHP {} available. Enable shell integration in the PHP section of the app.",
                    burd_php.version
                );
            } else {
                println!("      -> Download a PHP version in the Burd app's PHP section to use Burd's PHP CLI.");
            }
        }
    }

    if let Some(php_version) = &info.php_version {
        println!("PHP Require: {} (composer.json)", php_version);
    }

    // Database configuration
    println!();
    if let Some(db) = &info.database {
        println!("Database:");
        println!("  Connection: {}", db.connection);
        if !db.host.is_empty() {
            println!("  Host: {}:{}", db.host, db.port);
        }
        println!("  Database: {}", db.database);
        println!("  Username: {}", db.username);

        // Print database-related issues
        print_category_issues(info, "database");
    } else {
        println!("Database: Not configured");
        print_category_issues(info, "database");
    }

    // Cache configuration (Laravel only)
    if matches!(info.project_type, ProjectType::Laravel { .. }) {
        println!();
        if let Some(cache) = &info.cache {
            println!("Cache:");
            println!("  Driver: {}", cache.driver);
            if let Some(host) = &cache.host {
                println!("  Host: {}", host);
            }
            if let Some(port) = cache.port {
                println!("  Port: {}", port);
            }
            print_category_issues(info, "cache");
        } else {
            println!("Cache: Not configured");
        }
    }

    // Mail configuration (Laravel only)
    if matches!(info.project_type, ProjectType::Laravel { .. }) {
        println!();
        if let Some(mail) = &info.mail {
            println!("Mail:");
            println!("  Mailer: {}", mail.mailer);
            if mail.mailer == "smtp" {
                println!("  Host: {}:{}", mail.host, mail.port);
            }
            print_category_issues(info, "mail");
        } else {
            println!("Mail: Not configured");
        }
    }

    // Search configuration (Laravel only)
    if matches!(info.project_type, ProjectType::Laravel { .. }) {
        if let Some(search) = &info.search {
            println!();
            println!("Search:");
            println!("  Driver: {}", search.driver);
            if let Some(host) = &search.host {
                println!("  Host: {}", host);
            }
            print_category_issues(info, "search");
        }
    }

    // General issues
    let general_issues: Vec<_> = info
        .issues
        .iter()
        .filter(|i| {
            i.category == "config"
                || i.category == "project"
                || !["database", "cache", "mail", "search"].contains(&i.category.as_str())
        })
        .collect();

    if !general_issues.is_empty() {
        println!();
        println!("Issues:");
        for issue in general_issues {
            print_issue(issue);
        }
    }

    // Summary
    println!();
    let error_count = info
        .issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Error)
        .count();
    let warning_count = info
        .issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Warning)
        .count();
    let info_count = info
        .issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::Info)
        .count();

    if error_count > 0 || warning_count > 0 || info_count > 0 {
        println!(
            "Summary: {} error(s), {} warning(s), {} suggestion(s)",
            error_count, warning_count, info_count
        );
    } else {
        println!("No issues found.");
    }

    println!();
}

/// Print issues for a specific category
fn print_category_issues(info: &ProjectInfo, category: &str) {
    let issues = info.issues_for_category(category);
    for issue in issues {
        print_issue(issue);
    }
}

/// Print a single issue
fn print_issue(issue: &crate::analyzer::ProjectIssue) {
    let prefix = match issue.severity {
        IssueSeverity::Error => "  [!]",
        IssueSeverity::Warning => "  [*]",
        IssueSeverity::Info => "  [i]",
    };

    println!("{} {}", prefix, issue.message);

    if let Some(suggestion) = &issue.suggestion {
        println!("      -> {}", suggestion);
    }
}
