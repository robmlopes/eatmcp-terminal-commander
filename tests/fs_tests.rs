use std::fs;
use eatmcp::fs::{edit_file, read_file, search_files, write_file};

#[test]
fn test_write_and_read_file() {
    let test_dir = "tests/tmp_fs_test";
    let test_file = format!("{}/nested/hello.txt", test_dir);

    // 1. Write file with nested directory creation
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let write_res = write_file(&test_file, content, true);
    assert!(write_res.is_ok(), "write_file failed: {:?}", write_res);
    assert!(write_res.unwrap().contains("OK: wrote"));

    // 2. Read with offset and limit
    let read_res = read_file(&test_file, Some(2), Some(2), None);
    assert!(read_res.is_ok());
    let text = read_res.unwrap();
    assert_eq!(text, "2:line 2\n3:line 3\n[more lines: next offset=4]\n");

    // Read to the end (no truncation or more lines indicator)
    let read_end = read_file(&test_file, Some(4), Some(10), None);
    assert!(read_end.is_ok());
    assert_eq!(read_end.unwrap(), "4:line 4\n5:line 5\n");

    // 3. Read with byte cap truncation
    let read_capped = read_file(&test_file, Some(1), Some(10), Some(15));
    assert!(read_capped.is_ok());
    let capped_text = read_capped.unwrap();
    assert!(capped_text.contains("[truncated:"));

    // Cleanup
    let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_edit_file() {
    let test_dir = "tests/tmp_edit_test";
    let test_file = format!("{}/edit_target.txt", test_dir);
    let _ = fs::create_dir_all(test_dir);

    // Initial write
    let content = "foo bar foo\nsecond line\n";
    fs::write(&test_file, content).unwrap();

    // Ambiguous match without replace_all should fail
    let edit_fail = edit_file(&test_file, "foo", "baz", false);
    assert!(edit_fail.is_err());
    assert!(edit_fail.unwrap_err().contains("matched 2 times"));

    // replace_all = true should succeed
    let edit_all = edit_file(&test_file, "foo", "baz", true);
    assert!(edit_all.is_ok());
    let new_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(new_content, "baz bar baz\nsecond line\n");

    // Single exact match should succeed
    let edit_single = edit_file(&test_file, "bar", "qux", false);
    assert!(edit_single.is_ok());
    assert!(edit_single.unwrap().contains("1 match replaced"));
    let final_content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(final_content, "baz qux baz\nsecond line\n");

    // Not found
    let edit_not_found = edit_file(&test_file, "nonexistent", "x", false);
    assert!(edit_not_found.is_err());

    // Cleanup
    let _ = fs::remove_dir_all(test_dir);
}

#[test]
fn test_search_files() {
    let test_dir = "tests/tmp_search_test";
    let file1 = format!("{}/file1.rs", test_dir);
    let file2 = format!("{}/file2.txt", test_dir);
    let _ = fs::create_dir_all(test_dir);

    fs::write(&file1, "fn calculate_sum() -> i32 {\n    42\n}\n").unwrap();
    fs::write(&file2, "Documentation mentioning calculate_sum.\nOther text.\n").unwrap();

    // 1. Search with pattern and glob filter
    let res_glob = search_files(test_dir, "calculate_sum", Some("*.rs"), None, None);
    assert!(res_glob.is_ok());
    let text = res_glob.unwrap();
    assert!(text.contains("file1.rs:1:fn calculate_sum() -> i32 {"));
    assert!(!text.contains("file2.txt"));

    // 2. Search pattern across all files
    let res_all = search_files(test_dir, "calculate_sum", None, None, None);
    assert!(res_all.is_ok());
    let all_text = res_all.unwrap();
    assert!(all_text.contains("file1.rs"));
    assert!(all_text.contains("file2.txt"));

    // 3. Glob only (empty pattern)
    let res_paths = search_files(test_dir, "", Some("*.txt"), None, None);
    assert!(res_paths.is_ok());
    let paths_text = res_paths.unwrap();
    assert!(paths_text.contains("file2.txt"));
    assert!(!paths_text.contains("file1.rs"));

    // Cleanup
    let _ = fs::remove_dir_all(test_dir);
}
