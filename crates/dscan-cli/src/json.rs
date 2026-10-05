use std::fmt::Write;

use dscan_core::ScanResult;
use dscan_core::snapshot::build_extension_breakdown;

pub fn escape_json_str(s: &str, out: &mut String) {
    out.reserve(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

pub fn serialize_scan_result(target_path: &str, result: &ScanResult) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("{\n  \"target_path\": \"");
    escape_json_str(target_path, &mut out);
    let _ = write!(
        out,
        "\",\n  \"total_bytes\": {},\n  \"total_files\": {},\n  \"duration_ms\": {},\n  \"top_dirs\": [",
        result.total_bytes,
        result.total_files,
        result.elapsed.as_millis()
    );

    for (i, (path, bytes)) in result.top_dirs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n      \"path\": \"");
        escape_json_str(&path.display().to_string(), &mut out);
        let _ = write!(out, "\",\n      \"bytes\": {}\n    }}", bytes);
    }
    if !result.top_dirs.is_empty() {
        out.push_str("\n  ");
    }
    out.push_str("],\n  \"top_files\": [");

    for (i, (bytes, path)) in result.top_files.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n      \"path\": \"");
        escape_json_str(&path.display().to_string(), &mut out);
        let _ = write!(out, "\",\n      \"bytes\": {}\n    }}", bytes);
    }
    if !result.top_files.is_empty() {
        out.push_str("\n  ");
    }
    out.push_str("],\n  \"extension_stats\": [");

    // Collect all extensions without truncation for JSON export (limit = 0)
    let ext_list = build_extension_breakdown(&result.extension_stats, result.total_bytes, 0);
    for (i, item) in ext_list.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n    {\n      \"extension\": \"");
        escape_json_str(&item.extension, &mut out);
        let _ = write!(
            out,
            "\",\n      \"total_bytes\": {},\n      \"file_count\": {},\n      \"percentage\": {:.2}\n    }}",
            item.total_bytes, item.file_count, item.percentage_of_total
        );
    }
    if !ext_list.is_empty() {
        out.push_str("\n  ");
    }
    out.push_str("]\n}\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_special_characters() {
        let mut out = String::new();
        escape_json_str("hello \"world\" \\\n\r\t\x08\x0C\x01 test", &mut out);
        assert_eq!(out, "hello \\\"world\\\" \\\\\\n\\r\\t\\b\\f\\u0001 test");
    }

    #[test]
    fn test_serialize_empty_scan_result() {
        let res = ScanResult {
            elapsed: std::time::Duration::from_millis(100),
            total_bytes: 0,
            total_files: 0,
            top_dirs: Vec::new(),
            top_files: Vec::new(),
            max_dir_size: 0,
            max_file_size: 0,
            arenas: Vec::new(),
            root: std::path::PathBuf::from("."),
            extension_stats: std::collections::HashMap::new(),
        };
        let json = serialize_scan_result(".", &res);
        assert!(json.contains("\"target_path\": \".\""));
        assert!(json.contains("\"total_bytes\": 0"));
        assert!(json.contains("\"total_files\": 0"));
        assert!(json.contains("\"duration_ms\": 100"));
        assert!(json.contains("\"top_dirs\": []"));
        assert!(json.contains("\"top_files\": []"));
        assert!(json.contains("\"extension_stats\": []"));
    }

    #[test]
    fn test_serialize_scan_result_fields() {
        let mut ext_stats = std::collections::HashMap::new();
        ext_stats.insert(".rs".to_string(), (1024, 2));

        let res = ScanResult {
            elapsed: std::time::Duration::from_millis(250),
            total_bytes: 2048,
            total_files: 4,
            top_dirs: vec![(std::path::PathBuf::from("src"), 1024)],
            top_files: vec![(1024, std::path::PathBuf::from("src/main.rs"))],
            max_dir_size: 1024,
            max_file_size: 1024,
            arenas: Vec::new(),
            root: std::path::PathBuf::from("."),
            extension_stats: ext_stats,
        };
        let json = serialize_scan_result("test/path", &res);
        assert!(json.contains("\"target_path\": \"test/path\""));
        assert!(json.contains("\"total_bytes\": 2048"));
        assert!(json.contains("\"total_files\": 4"));
        assert!(json.contains("\"duration_ms\": 250"));
        assert!(json.contains("\"path\": \"src\""));
        assert!(json.contains("\"bytes\": 1024"));
        assert!(json.contains("\"path\": \"src/main.rs\""));
        assert!(json.contains("\"extension\": \".rs\""));
        assert!(json.contains("\"percentage\": 50.00"));
    }
}
