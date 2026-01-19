use crate::widgets::{Cheat, SearchBar};



pub fn build_search_bar() -> SearchBar {
    use crate::app::config_system::ConfigStore;

    let mut cheats: Vec<Cheat> = vec![];

    // Load schema and auto-generate searchable cheats
    if let Ok(config_store) = ConfigStore::load() {
        let schema = config_store.schema();
        for (section_name, section) in &schema.sections {
            for (field_name, field_schema) in &section.fields {
                // Only include public fields in search
                if field_schema.public {
                    let key = format!("{}.{}", section_name, field_name);

                    // Generate tags from field name and category
                    let mut tags: Vec<String> = vec![
                        section_name.clone().to_lowercase(),
                        field_schema.metadata.category.clone().to_lowercase(),
                    ];

                    // Add words from display name as tags
                    for word in field_schema.metadata.display_name.split_whitespace() {
                        tags.push(word.to_string().to_lowercase());
                    }

                    cheats.push(Cheat {
                        id: key,
                        display_name: field_schema.metadata.display_name.clone(),
                        description: if !field_schema.metadata.description.is_empty() {
                            field_schema.metadata.description.clone()
                        } else {
                            field_schema.metadata.tooltip.clone()
                        },
                        category: section.display_name.clone(),
                        tags,
                    });
                }
            }
        }
    }

    SearchBar::new(cheats)
}
