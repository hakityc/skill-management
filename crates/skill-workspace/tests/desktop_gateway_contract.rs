use serde_json::json;
use skill_workspace::{SkillDraft, SkillDraftTarget, SkillFilePreview};

#[test]
fn desktop_gateway_accepts_camel_case_fields_inside_draft_target_variants() {
    let draft: SkillDraft = serde_json::from_value(json!({
        "target": { "kind": "existing", "instanceId": "instance-1" },
        "name": "release-notes",
        "description": "整理版本发布说明。",
        "markdownBody": "# 发布说明\n",
        "fileChanges": []
    }))
    .expect("反序列化桌面网关 Skill 草稿");

    assert_eq!(
        draft.target,
        SkillDraftTarget::Existing {
            instance_id: "instance-1".to_owned()
        }
    );
}

#[test]
fn desktop_gateway_emits_camel_case_fields_inside_file_preview_variants() {
    let value = serde_json::to_value(SkillFilePreview::Binary {
        size: 3,
        media_type: Some("image/png".to_owned()),
        preview_content: Some(vec![1, 2, 3]),
    })
    .expect("序列化桌面网关文件预览");

    assert_eq!(
        value,
        json!({
            "kind": "binary",
            "size": 3,
            "mediaType": "image/png",
            "previewContent": [1, 2, 3]
        })
    );
}
