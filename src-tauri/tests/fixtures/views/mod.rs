use knowledge_garden::application::views::ViewDefinition;
use serde_json::json;
use std::{fs, path::Path};

pub fn seed(root: &Path, name: &str, definition: &ViewDefinition) -> String {
    let id = format!("view-{}", uuid::Uuid::new_v4());
    let record = json!({"schema": 1, "view_id": id, "name": name, "definition": definition});
    fs::create_dir_all(root.join("views")).unwrap();
    fs::write(
        root.join("views").join(format!("{id}.md")),
        format!(
            "---\n{}---\n# {name}\n\nLegacy owner content.\n",
            serde_yaml_ng::to_string(&record).unwrap()
        ),
    )
    .unwrap();
    id
}
