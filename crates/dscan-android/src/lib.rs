use std::fmt::Write as _;
use std::sync::Arc;

use dscan_core::ScanOptions;
use dscan_core::snapshot::ScanSession;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jstring};

fn escape_json_str(s: &str, out: &mut String) {
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

/// Start an asynchronous disk scan session.
/// Returns a raw pointer cast to jlong, or 0 on error.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_startScan(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
    threads: jint,
) -> jlong {
    let path_str: String = match env.get_string(&path) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let p = std::path::Path::new(&path_str);
    let auto_threads = if threads > 0 {
        threads as usize
    } else {
        ScanOptions::auto_threads_for_path(p)
    };

    let opts = ScanOptions {
        target_path: path_str,
        top_limit: 500,
        max_depth: usize::MAX,
        threads: auto_threads,
        excludes: vec![
            "/proc".to_string(),
            "/sys".to_string(),
            "/dev".to_string(),
            ".git".to_string(),
        ],
        follow_symlinks: false,
        cross_filesystems: false,
        collect_ext_stats: true,
    };

    match ScanSession::start(opts) {
        Ok(session) => {
            let boxed = Box::new(session);
            Box::into_raw(boxed) as jlong
        }
        Err(_) => 0,
    }
}

/// Poll the scan progress of an active session.
/// Returns a JSON string with progress metrics.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_pollProgress(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("{}")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let prog = session.poll_progress();

    let mut json = String::with_capacity(512);
    json.push_str("{\"total_bytes\":");
    let _ = write!(json, "{}", prog.total_bytes);
    json.push_str(",\"total_files\":");
    let _ = write!(json, "{}", prog.total_files);
    json.push_str(",\"active_workers\":");
    let _ = write!(json, "{}", prog.active_workers);
    json.push_str(",\"elapsed_millis\":");
    let _ = write!(json, "{}", prog.elapsed_millis);
    json.push_str(",\"files_per_sec\":");
    let _ = write!(json, "{:.2}", prog.files_per_sec);
    json.push_str(",\"bytes_per_sec\":");
    let _ = write!(json, "{:.2}", prog.bytes_per_sec);
    json.push_str(",\"is_complete\":");
    json.push_str(if prog.is_complete { "true" } else { "false" });
    json.push_str(",\"current_path\":\"");
    escape_json_str(&prog.current_path, &mut json);
    json.push_str("\"}");

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Retrieve hierarchical treemap nodes as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_getTreemapNodes(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
    max_depth: jint,
    max_nodes: jint,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("[]")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let depth = if max_depth > 0 { max_depth as u16 } else { 6 };
    let count = if max_nodes > 0 {
        max_nodes as usize
    } else {
        2048
    };

    let nodes = session.get_hierarchical_view(depth, count);

    let mut json = String::with_capacity(nodes.len() * 128 + 16);
    json.push('[');

    for (i, node) in nodes.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"id\":");
        let _ = write!(json, "{}", node.id);
        json.push_str(",\"parent_id\":");
        let _ = write!(json, "{}", node.parent_id);
        json.push_str(",\"name\":\"");
        escape_json_str(&node.name, &mut json);
        json.push_str("\",\"total_bytes\":");
        let _ = write!(json, "{}", node.total_bytes);
        json.push_str(",\"direct_bytes\":");
        let _ = write!(json, "{}", node.direct_bytes);
        json.push_str(",\"rel_depth\":");
        let _ = write!(json, "{}", node.rel_depth);
        json.push_str(",\"is_dir\":");
        json.push_str(if node.is_dir { "true" } else { "false" });
        json.push_str(",\"extension\":\"");
        escape_json_str(&node.extension, &mut json);
        json.push_str("\",\"children_ids\":[");
        for (ci, &cid) in node.children_ids.iter().enumerate() {
            if ci > 0 {
                json.push(',');
            }
            let _ = write!(json, "{}", cid);
        }
        json.push_str("]}");
    }

    json.push(']');

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Retrieve file extension breakdown statistics as JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_getExtensionBreakdown(
    env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
    limit: jint,
) -> jstring {
    if session_ptr == 0 {
        return env
            .new_string("[]")
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut());
    }

    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    let lim = if limit > 0 { limit as usize } else { 16 };
    let exts = session.get_extension_breakdown(lim);

    let mut json = String::with_capacity(exts.len() * 96 + 16);
    json.push('[');

    for (i, ext) in exts.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str("{\"extension\":\"");
        escape_json_str(&ext.extension, &mut json);
        json.push_str("\",\"total_bytes\":");
        let _ = write!(json, "{}", ext.total_bytes);
        json.push_str(",\"file_count\":");
        let _ = write!(json, "{}", ext.file_count);
        json.push_str(",\"percentage\":");
        let _ = write!(json, "{:.2}", ext.percentage_of_total);
        json.push('}');
    }

    json.push(']');

    env.new_string(&json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Request cancellation of the active scan.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_cancelScan(
    _env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) {
    if session_ptr == 0 {
        return;
    }
    let session: &Arc<ScanSession> = unsafe { &*(session_ptr as *const Arc<ScanSession>) };
    session.cancel();
}

/// Stop scan and safely free the session Arc pointer.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_dscan_app_DscanBridge_stopScan(
    _env: JNIEnv,
    _class: JClass,
    session_ptr: jlong,
) {
    if session_ptr == 0 {
        return;
    }
    let boxed = unsafe { Box::from_raw(session_ptr as *mut Arc<ScanSession>) };
    boxed.cancel();
    drop(boxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json_str() {
        let mut out = String::new();
        escape_json_str("hello \"world\"\n\t\\", &mut out);
        assert_eq!(out, "hello \\\"world\\\"\\n\\t\\\\");
    }
}
