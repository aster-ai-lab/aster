use super::*;
use std::cell::Cell;

#[test]
fn source_mutation_admits_a_large_vault_without_the_maintenance_512_file_scan() {
    let vault = Vault::new();
    for number in 0..5_000 {
        vault.write(&format!("notes/{number:04}.md"), "Unchanged source.\n");
    }
    let result = with_knowledge_source_mutation(&vault.0, |root| {
        assert_eq!(root, vault.0);
        fs::write(root.join("notes/4999.md"), "Reviewed mutation.\n")?;
        Ok(5_000)
    })
    .unwrap();
    assert_eq!(result, 5_000);
    assert_eq!(
        fs::read_to_string(vault.0.join("notes/4999.md")).unwrap(),
        "Reviewed mutation.\n"
    );
    assert!(!vault.0.join(STATE).exists());
}

#[test]
fn working_and_pending_review_jobs_refuse_before_the_source_callback() {
    let vault = Vault::new();
    vault.write("meeting.md", "New evidence.\n");
    vault.write(
        "concepts/shared.md",
        "---\ntype: Concept\ntitle: Prior\n---\n\nPrior conclusion.\n",
    );
    let job = vault.begin();
    let called = Cell::new(false);
    let error = with_knowledge_source_mutation(&vault.0, |_| {
        called.set(true);
        Ok(())
    })
    .unwrap_err();
    assert!(!called.get());
    let blocked = error
        .get_ref()
        .unwrap()
        .downcast_ref::<KnowledgeSourceMutationBlocked>()
        .unwrap();
    assert_eq!(blocked.job_id, job.id);
    assert_eq!(blocked.sources, vec!["meeting.md"]);
    assert!(blocked.pages.is_empty());
    vault.propose(&job, "update");
    finish_knowledge_maintenance(&vault.0, &job.id).unwrap();
    let error = with_knowledge_source_mutation(&vault.0, |_| {
        called.set(true);
        Ok(())
    })
    .unwrap_err();
    assert!(!called.get());
    let blocked = error
        .get_ref()
        .unwrap()
        .downcast_ref::<KnowledgeSourceMutationBlocked>()
        .unwrap();
    assert_eq!(blocked.pages, vec!["concepts/shared.md"]);
    assert_eq!(
        fs::read_to_string(vault.0.join("meeting.md")).unwrap(),
        "New evidence.\n"
    );
    abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
}

#[test]
fn a_source_callback_holds_the_existing_publication_lock() {
    let vault = Vault::new();
    vault.write("meeting.md", "Evidence.\n");
    with_knowledge_source_mutation(&vault.0, |_| {
        let error =
            begin_knowledge_maintenance(&vault.0, &[], MaintenanceOperation::Ingest).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        Ok(())
    })
    .unwrap();
    let job = vault.begin();
    assert!(job.staging.contains("wiki/local"));
    abandon_knowledge_maintenance(&vault.0, &job.id).unwrap();
}
