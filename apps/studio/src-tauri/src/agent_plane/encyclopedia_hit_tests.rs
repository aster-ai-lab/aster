use serde_json::json;

use super::passage_hit;

#[test]
fn a_hybrid_hit_names_the_file_passage_and_generation() {
    let hit = passage_hit(&json!({
        "id": "6f1c1f1e-0000-4000-8000-000000000001",
        "passage_id": "6f1c1f1e-0000-4000-8000-000000000001",
        "file_id": "6f1c1f1e-0000-4000-8000-0000000000aa",
        "generation": 3,
        "heading": "## 差旅票据提交时限",
        "folder_name": "制度",
        "body": "出差结束后十个工作日内提交。",
        "score": 0.5
    }))
    .expect("hit");
    assert_eq!(hit.name, "差旅票据提交时限");
    assert_eq!(
        hit.place,
        "team/6f1c1f1e-0000-4000-8000-0000000000aa/6f1c1f1e-0000-4000-8000-000000000001@3"
    );
}

#[test]
fn an_old_scorer_hit_keeps_the_folder_and_a_forged_id_is_not_a_citation() {
    let old = passage_hit(&json!({
        "id": "6f1c1f1e-0000-4000-8000-000000000001",
        "heading": "考勤",
        "folder_name": "制度",
        "body": "九点前打卡。",
        "generation": 1
    }))
    .expect("hit");
    assert_eq!(old.place, "制度");
    let forged = passage_hit(&json!({
        "passage_id": "../../etc",
        "file_id": "6f1c1f1e-0000-4000-8000-0000000000aa",
        "generation": 1,
        "heading": "考勤",
        "folder_name": "",
        "id": "x",
        "body": "九点前打卡。"
    }))
    .expect("hit");
    assert_eq!(forged.place, "x");
}
