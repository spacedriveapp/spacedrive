//! Integration tests for the indexing rules engine

use sd_core::ops::indexing::rules::{
	build_default_ruler, IndexerRule, IndexerRuler, RulePerKind, RuleToggles, RulerDecision,
};
use std::collections::HashSet;
use tempfile::TempDir;

fn touch(path: &std::path::Path) {
	std::fs::create_dir_all(path.parent().unwrap()).ok();
	std::fs::write(path, b"").unwrap();
}

#[tokio::test]
async fn test_gitignore_basic() {
	let tmp = TempDir::new().unwrap();
	let root = tmp.path();

	// Simulate a git repo
	std::fs::create_dir_all(root.join(".git")).unwrap();

	// Create a directory with a .gitignore
	let dir = root.join("a");
	std::fs::create_dir_all(&dir).unwrap();
	let gitignore = dir.join(".gitignore");
	std::fs::write(&gitignore, b"*.log\n!keep.log\n").unwrap();

	let f_ignored = dir.join("test.log");
	let f_kept = dir.join("keep.log");
	touch(&f_ignored);
	touch(&f_kept);

	let toggles = RuleToggles {
		no_system_files: false,
		no_hidden: false,
		no_git: false,
		gitignore: true,
		only_images: false,
		no_dev_dirs: false,
	};
	// current should be the directory containing the files (like discovery)
	let ruler = build_default_ruler(toggles, root, &dir).await;

	let d1 = ruler
		.evaluate_path(&f_ignored, &std::fs::metadata(&f_ignored).unwrap())
		.await
		.unwrap();
	assert!(
		matches!(d1, RulerDecision::Reject),
		"ignored file should reject"
	);

	let d2 = ruler
		.evaluate_path(&f_kept, &std::fs::metadata(&f_kept).unwrap())
		.await
		.unwrap();
	assert!(
		matches!(d2, RulerDecision::Accept),
		"negated pattern should accept"
	);
}

#[tokio::test]
async fn test_conflict_precedence_and_children_rules() {
	let tmp = TempDir::new().unwrap();
	let root = tmp.path();

	let file_txt = root.join("doc.txt");
	touch(&file_txt);

	// Build a custom ruler with both accept and reject globs matching *.txt
	let accept = RulePerKind::new_accept_files_by_globs_str(["**/*.txt"]).unwrap();
	let reject = RulePerKind::new_reject_files_by_globs_str(["**/*.txt"]).unwrap();
	let rule = IndexerRule {
		id: None,
		name: "conflict".to_string(),
		default: true,
		rules: vec![accept, reject],
		date_created: chrono::Utc::now(),
		date_modified: chrono::Utc::now(),
	};
	let ruler = IndexerRuler::new(vec![rule]);

	let d = ruler
		.evaluate_path(&file_txt, &std::fs::metadata(&file_txt).unwrap())
		.await
		.unwrap();
	// Reject should win over accept
	assert!(matches!(d, RulerDecision::Reject));

	// Children directory rules
	let dir = root.join("project");
	std::fs::create_dir_all(dir.join("src")).unwrap();

	let mut set: HashSet<String> = HashSet::new();
	set.insert("src".to_string());
	let child_reject = RulePerKind::RejectIfChildrenDirectoriesArePresent(set.clone());
	let rule_children = IndexerRule {
		id: None,
		name: "child_reject".to_string(),
		default: true,
		rules: vec![child_reject],
		date_created: chrono::Utc::now(),
		date_modified: chrono::Utc::now(),
	};
	let ruler_children = IndexerRuler::new(vec![rule_children]);
	// evaluate_path currently doesn't consider is_dir for children rules in reject_path.
	// Instead, inspect the acceptance map directly and assert the helper flags rejection.
	let acc = ruler_children
		.apply_all(&dir, &std::fs::metadata(&dir).unwrap())
		.await
		.unwrap();
	assert!(sd_core::ops::indexing::rules::IndexerRuler::rejected_by_children_directories(&acc));

	// A directory without the child should be accepted
	let other = root.join("other");
	std::fs::create_dir_all(&other).unwrap();
	let acc_other = ruler_children
		.apply_all(&other, &std::fs::metadata(&other).unwrap())
		.await
		.unwrap();
	assert!(
		!sd_core::ops::indexing::rules::IndexerRuler::rejected_by_children_directories(&acc_other)
	);
}

#[tokio::test]
async fn test_rule_toggles_basic() {
	let tmp = TempDir::new().unwrap();
	let root = tmp.path();

	// Files and dirs
	let path_normal = root.join("file.txt");
	touch(&path_normal);

	let path_hidden = root.join(".hidden");
	touch(&path_hidden);

	let img_jpg = root.join("photo.jpg");
	touch(&img_jpg);

	let dev_dir = root.join("node_modules");
	std::fs::create_dir_all(&dev_dir).unwrap();

	// 1) no_dev_dirs = true should reject node_modules directory
	let toggles = RuleToggles {
		no_system_files: false,
		no_hidden: false,
		no_git: false,
		gitignore: false,
		only_images: false,
		no_dev_dirs: true,
	};
	let ruler = build_default_ruler(toggles, root, root).await;
	let decision = ruler
		.evaluate_path(&dev_dir, &std::fs::metadata(&dev_dir).unwrap())
		.await
		.unwrap();
	assert!(matches!(decision, RulerDecision::Reject));

	// Normal file should be accepted
	let decision = ruler
		.evaluate_path(&path_normal, &std::fs::metadata(&path_normal).unwrap())
		.await
		.unwrap();
	assert!(matches!(decision, RulerDecision::Accept));

	// 2) no_hidden = true should reject hidden files
	let toggles = RuleToggles {
		no_system_files: false,
		no_hidden: true,
		no_git: false,
		gitignore: false,
		only_images: false,
		no_dev_dirs: false,
	};
	let ruler = build_default_ruler(toggles, root, root).await;
	let decision = ruler
		.evaluate_path(&path_hidden, &std::fs::metadata(&path_hidden).unwrap())
		.await
		.unwrap();
	assert!(matches!(decision, RulerDecision::Reject));

	// 3) only_images = true should accept images and reject non-images
	let toggles = RuleToggles {
		no_system_files: false,
		no_hidden: false,
		no_git: false,
		gitignore: false,
		only_images: true,
		no_dev_dirs: false,
	};
	let ruler = build_default_ruler(toggles, root, root).await;
	let decision_img = ruler
		.evaluate_path(&img_jpg, &std::fs::metadata(&img_jpg).unwrap())
		.await
		.unwrap();
	assert!(matches!(decision_img, RulerDecision::Accept));
	let decision_txt = ruler
		.evaluate_path(&path_normal, &std::fs::metadata(&path_normal).unwrap())
		.await
		.unwrap();
	assert!(matches!(decision_txt, RulerDecision::Reject));

	// 4) no_system_files = true should reject platform-specific system files
	let toggles = RuleToggles {
		no_system_files: true,
		no_hidden: false,
		no_git: false,
		gitignore: false,
		only_images: false,
		no_dev_dirs: false,
	};
	let ruler = build_default_ruler(toggles, root, root).await;
	#[cfg(target_os = "macos")]
	{
		let ds_store = root.join(".DS_Store");
		touch(&ds_store);
		let decision = ruler
			.evaluate_path(&ds_store, &std::fs::metadata(&ds_store).unwrap())
			.await
			.unwrap();
		assert!(matches!(decision, RulerDecision::Reject));
	}
	#[cfg(target_os = "windows")]
	{
		let thumbs = root.join("Thumbs.db");
		touch(&thumbs);
		let decision = ruler
			.evaluate_path(&thumbs, &std::fs::metadata(&thumbs).unwrap())
			.await
			.unwrap();
		assert!(matches!(decision, RulerDecision::Reject));
	}
}
