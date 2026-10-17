//! Guard the pre-release implementation backlog against missing child files,
//! dependency cycles and unmapped parent acceptance criteria.
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn frontmatter(document: &str) -> &str {
    document
        .split("---\n")
        .nth(1)
        .expect("task YAML frontmatter")
}

fn dependencies(document: &str) -> Vec<String> {
    let mut collecting = false;
    let mut out = Vec::new();
    for line in frontmatter(document).lines() {
        if line.starts_with("dependencies:") {
            collecting = true;
        } else if collecting {
            if let Some(dep) = line.strip_prefix("  - TASK-") {
                out.push(format!("TASK-{dep}"));
            } else if !line.trim().is_empty() {
                break;
            }
        }
    }
    out
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut separated = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            if separated && !out.is_empty() {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
            separated = false;
        } else {
            separated = true;
        }
    }
    out
}

fn acceptance_numbers(text: &str) -> BTreeSet<u64> {
    let section = text
        .split("<!-- AC:BEGIN -->")
        .nth(1)
        .expect("AC begin")
        .split("<!-- AC:END -->")
        .next()
        .expect("AC end");
    section
        .lines()
        .filter_map(|line| line.split("] #").nth(1))
        .map(|tail| {
            tail.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .expect("numbered acceptance criterion")
        })
        .collect()
}

#[test]
fn all_frozen_parent_queues_have_acyclic_child_dependencies_and_complete_ac_mapping() {
    let plan: serde_json::Value =
        serde_json::from_str(include_str!("../docs/backlog-child-queue-plan.json"))
            .expect("versioned child plan");
    let parents = plan["parents"].as_array().expect("parent plan");
    assert_eq!(
        parents.len(),
        19,
        "all unfinished pre-v3 implementation parents"
    );
    let mut registered = BTreeSet::new();
    let mut global_ids = Vec::new();
    for parent in parents {
        let number = parent["parent"].as_u64().expect("parent number");
        assert!((71..=90).contains(&number) && number != 79);
        let entries = fs::read_dir(repo_path(".backlog/tasks"))
            .expect("backlog directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        let filename = entries
            .iter()
            .find(|name| name.starts_with(&format!("task-{number} - ")))
            .expect("parent file");
        let document = fs::read_to_string(repo_path(&format!(".backlog/tasks/{filename}")))
            .expect("parent contents");
        let children = parent["phases"].as_array().expect("bounded phases");
        assert!(!children.is_empty());
        let all_child_ids: BTreeSet<_> = children
            .iter()
            .map(|phase| format!("TASK-{}", phase["id"].as_u64().expect("child id")))
            .collect();
        let parent_deps = dependencies(&document);
        let upstream: Vec<_> = parent_deps
            .iter()
            .filter(|dep| !all_child_ids.contains(*dep))
            .cloned()
            .collect();
        for child in &all_child_ids {
            assert!(
                parent_deps.contains(child),
                "parent {number} missing closure dependency {child}"
            );
        }
        let original_ac = acceptance_numbers(&document);
        assert!(
            !original_ac.is_empty(),
            "parent {number} has no original AC"
        );
        let mut mapped = BTreeSet::new();
        for (i, phase) in children.iter().enumerate() {
            let child_id = phase["id"].as_u64().expect("child number");
            let child = format!("TASK-{child_id}");
            assert!(registered.insert(child.clone()), "duplicate {child}");
            global_ids.push(child_id);
            let name = format!(
                "task-{child_id} - {}.md",
                slug(phase["title"].as_str().expect("child title"))
            );
            let content = fs::read_to_string(repo_path(&format!(".backlog/tasks/{name}")))
                .unwrap_or_else(|error| panic!("missing {name}: {error}"));
            assert!(frontmatter(&content).contains(&format!("id: {child}")));
            assert!(frontmatter(&content).contains("status: To Do"));
            let expected = if i == 0 {
                upstream.clone()
            } else {
                vec![format!(
                    "TASK-{}",
                    children[i - 1]["id"].as_u64().expect("previous child")
                )]
            };
            assert_eq!(
                dependencies(&content),
                expected,
                "{child} has wrong dependencies"
            );
            assert!(!dependencies(&content).contains(&format!("TASK-{number}")));
            let criteria = acceptance_numbers(&content);
            assert_eq!(criteria, BTreeSet::from([1, 2, 3, 4, 5]), "{child}");
            assert!(
                !content.contains("- [x] #"),
                "{child} claimed Done prematurely"
            );
            let fixtures = phase["fixture_ids"].as_array().expect("fixture IDs");
            assert_eq!(fixtures.len(), 2);
            for fixture in fixtures {
                let id = fixture.as_str().expect("fixture ID");
                assert!(content.contains(id), "{child} missing {id}");
            }
            for original in phase["original_ac"].as_array().expect("parent AC mapping") {
                let ac = original.as_u64().expect("AC number");
                assert!(
                    original_ac.contains(&ac),
                    "{child} maps nonexistent parent AC #{ac}"
                );
                mapped.insert(ac);
            }
            for section in [
                "fixture IDs",
                "DuckDB",
                "Residual",
                "13",
                "docs/coverage-manifest.json",
            ] {
                assert!(
                    content.contains(section),
                    "{child} missing evidence gate {section}"
                );
            }
        }
        assert_eq!(
            mapped, original_ac,
            "parent {number} has unmapped original AC"
        );
    }
    global_ids.sort_unstable();
    assert_eq!(global_ids, (109..=188).collect::<Vec<_>>());
}
