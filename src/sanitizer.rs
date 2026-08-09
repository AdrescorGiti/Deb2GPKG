use std::collections::HashMap;

pub struct DependencySanitizer {
    mappings: HashMap<&'static str, &'static str>,
}

impl DependencySanitizer {
    pub fn new() -> Self {
        let mut mappings = HashMap::new();
        
        // 🔥 БЕЛЫЙ СПИСОК (WHITELIST) 🔥
        // Сюда добавляем ТОЛЬКО то, что реально существует как .gpkg в твоем репозитории
        mappings.insert("libssl-dev", "openssl");
        
        // Пример: если когда-нибудь сделаешь свой пакет драйверов, раскомментируешь
        // mappings.insert("nvidia-driver", "g-os-nvidia-drivers");

        Self { mappings }
    }

    pub fn sanitize(&self, raw_deps: Vec<String>) -> Vec<String> {
        let mut cleaned_deps = Vec::new();

        for dep in raw_deps {
            // Разбиваем альтернативные зависимости через '|'
            let choices: Vec<&str> = dep.split('|').map(|s| s.trim()).collect();
            
            for choice in choices {
                // Извлекаем чистое имя пакета без версий ("pkexec (>= 0.96)" -> "pkexec")
                let clean_name = choice.split_whitespace().next().unwrap_or(choice);

                // 🔥 ЖЕСТКАЯ ФИЛЬТРАЦИЯ 🔥
                // Если пакета НЕТ в словаре mappings (то есть G OS о нём не знает),
                // мы его просто ВЫКИДЫВАЕМ. Никаких блэклистов. Нет в маппинге — идет лесом.
                if let Some(&mapped) = self.mappings.get(clean_name) {
                    let mapped_str = mapped.to_string();
                    if !cleaned_deps.contains(&mapped_str) {
                        cleaned_deps.push(mapped_str);
                    }
                    break; // Нашли известную зависимость, сохраняем и идем к следующей строке
                }
            }
        }

        cleaned_deps
    }
}