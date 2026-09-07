use std::fs;

use health_engine::{SourceContentState, SourceDescriptor, SubjectId, Workspace};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn identical_source_bytes_reuse_an_opaque_source_identity() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    fs::write(
        root.path().join("sources/lab-a.txt"),
        b"synthetic-source-v1\n",
    )?;
    fs::write(
        root.path().join("sources/lab-copy.txt"),
        b"synthetic-source-v1\n",
    )?;

    let first = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;
    let second = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-copy.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;

    assert_eq!(second.source_id, first.source_id);
    assert!(!first.source_id.as_str().is_empty());

    let status = workspace.sources().status(&first.source_id)?;
    assert_eq!(status.content_state, SourceContentState::Available);
    assert_eq!(status.size_bytes, 20);
    assert_eq!(status.media_type, "text/plain");
    assert_eq!(
        status.aliases,
        ["sources/lab-a.txt", "sources/lab-copy.txt"]
    );

    let public_json = serde_json::to_string(&status)?;
    assert!(!public_json.contains("sha256"));
    assert!(!public_json.contains("digest"));
    assert!(!public_json.contains("synthetic-source-v1"));
    assert!(!public_json.contains(root.path().to_string_lossy().as_ref()));

    Ok(())
}

#[test]
fn source_status_reports_changed_content() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let alias = root.path().join("sources/lab-a.txt");
    fs::write(&alias, b"synthetic-source-v1\n")?;
    let registration = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;

    fs::write(&alias, b"synthetic-source-v2\n")?;
    let status = workspace.sources().status(&registration.source_id)?;

    assert_eq!(status.source_id, registration.source_id);
    assert_eq!(status.content_state, SourceContentState::Changed);
    assert_eq!(status.size_bytes, 20);
    assert_eq!(status.aliases, vec!["sources/lab-a.txt"]);
    let public_json = serde_json::to_string(&status)?;
    assert!(!public_json.contains("synthetic-source-v2"));
    assert!(!public_json.contains("digest"));
    assert!(!public_json.contains("sha256"));
    assert!(!public_json.contains(root.path().to_string_lossy().as_ref()));

    Ok(())
}

#[test]
fn source_status_reports_unavailable_content() -> TestResult {
    let root = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let alias = root.path().join("sources/lab-a.txt");
    fs::write(&alias, b"synthetic-source-v1\n")?;
    let registration = workspace.sources().register(SourceDescriptor {
        alias: "sources/lab-a.txt".to_owned(),
        media_type: "text/plain".to_owned(),
    })?;

    fs::remove_file(alias)?;
    let status = workspace.sources().status(&registration.source_id)?;

    assert_eq!(status.source_id, registration.source_id);
    assert_eq!(status.content_state, SourceContentState::Unavailable);
    assert_eq!(status.size_bytes, 20);
    assert_eq!(status.media_type, "text/plain");
    assert_eq!(status.aliases, vec!["sources/lab-a.txt"]);

    Ok(())
}

#[cfg(unix)]
#[test]
fn source_registration_rejects_alias_resolving_outside_workspace() -> TestResult {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let workspace = Workspace::init(root.path(), SubjectId::parse("subject-synthetic-001")?)?;
    fs::create_dir(root.path().join("sources"))?;
    let external_source = outside.path().join("external.txt");
    fs::write(&external_source, b"synthetic-external-source\n")?;
    symlink(&external_source, root.path().join("sources/escape.txt"))?;

    let error = workspace
        .sources()
        .register(SourceDescriptor {
            alias: "sources/escape.txt".to_owned(),
            media_type: "text/plain".to_owned(),
        })
        .expect_err("an alias escaping the Workspace must be rejected");

    assert_eq!(error.code(), "invalid_source_alias");
    let diagnostic = error.to_string();
    assert!(!diagnostic.contains("synthetic-external-source"));
    assert!(!diagnostic.contains(outside.path().to_string_lossy().as_ref()));

    Ok(())
}
