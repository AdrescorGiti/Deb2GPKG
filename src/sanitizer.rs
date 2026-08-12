use std::collections::HashMap;

/// Translates upstream dependency names into GValli's internal package namespace.
///
/// # Policy (per the d2g stability directive)
/// - Known mappings are applied deterministically.
/// - Unknown dependencies are **not** silently dropped. They are normalized
///   (version constraints, architecture qualifiers and `:amd64` suffixes
///   stripped) and kept in the dependency list, so GValli's resolver can
///   surface a clear "unmet dependency" error at install time instead of the
///   app silently failing to run. This is the strict-mapping fallback.
/// - A small alias table collapses the common Debian ⇄ Arch ⇄ Fedora naming
///   differences (e.g. `libssl1.1` / `openssl-1.1` / `openssl`).
pub struct DependencySanitizer {
    aliases: HashMap<&'static str, &'static str>,
    whitelist: HashMap<&'static str, &'static str>,
}

impl DependencySanitizer {
    pub fn new() -> Self {
        let mut aliases = HashMap::new();
        // Debian ⇄ Arch ⇄ Fedora canonicalization. The value is the GValli name.
        aliases.insert("libssl-dev", "openssl");
        aliases.insert("libssl1.1", "openssl");
        aliases.insert("libssl1.0", "openssl");
        aliases.insert("openssl-1.1", "openssl");
        aliases.insert("libc6", "glibc");
        aliases.insert("glibc", "glibc");
        aliases.insert("libstdc++6", "libstdc++");
        aliases.insert("libstdc++", "libstdc++");
        aliases.insert("libgcc-s1", "libgcc");
        aliases.insert("libgcc", "libgcc");
        aliases.insert("zlib1g", "zlib");
        aliases.insert("zlib", "zlib");
        aliases.insert("libbz2-1.0", "bzip2");
        aliases.insert("bzip2", "bzip2");
        aliases.insert("liblzma5", "xz");
        aliases.insert("xz-utils", "xz");
        aliases.insert("xz", "xz");
        aliases.insert("libzstd1", "zstd");
        aliases.insert("zstd", "zstd");
        aliases.insert("libexpat1", "expat");
        aliases.insert("expat", "expat");
        aliases.insert("libffi7", "libffi");
        aliases.insert("libffi", "libffi");
        aliases.insert("libpcre3", "pcre");
        aliases.insert("pcre", "pcre");

        // Whitelist of packages that exist as .gpkg in the G OS repo and may
        // therefore be emitted as hard dependencies. Anything not here is
        // still emitted (strict fallback) but GValli will flag it.
        let mut whitelist = HashMap::new();
        for &v in aliases.values() {
            whitelist.insert(v, v);
        }
        // Extra G OS-native packages with no upstream alias.
        whitelist.insert("g-os-base", "g-os-base");
        whitelist.insert("gvalli", "gvalli");

        Self { aliases, whitelist }
    }

    pub fn sanitize(&self, raw_deps: Vec<String>) -> Vec<String> {
        let mut cleaned_deps: Vec<String> = Vec::with_capacity(raw_deps.len());

        for dep in raw_deps {
            // Split Debian-style alternatives "a | b" — keep the first known.
            let choices: Vec<&str> = dep.split('|').map(|s| s.trim()).collect();

            let mut resolved: Option<String> = None;
            for choice in &choices {
                let clean_name = Self::strip_constraint(choice);

                // 1. Direct alias hit.
                if let Some(&mapped) = self.aliases.get(clean_name.as_str()) {
                    resolved = Some(mapped.to_string());
                    break;
                }
                // 2. Already a whitelisted G OS name.
                if self.whitelist.contains_key(clean_name.as_str()) {
                    resolved = Some(clean_name);
                    break;
                }
            }

            // 3. Strict fallback: if no alternative resolved, keep the first
            //    choice's clean name so GValli can report it rather than the
            //    app silently missing a library at runtime.
            let final_dep = resolved.unwrap_or_else(|| {
                Self::strip_constraint(choices.first().copied().unwrap_or(""))
            });

            if !final_dep.is_empty() && !cleaned_deps.contains(&final_dep) {
                cleaned_deps.push(final_dep);
            }
        }

        cleaned_deps
    }

    /// Reduce "libfoo (>= 1.2)" / "libfoo:amd64" / "libfoo>=1.2" to "libfoo".
    fn strip_constraint(raw: &str) -> String {
        let s = raw.split(':').next().unwrap_or(raw).trim();
        if let Some(idx) = s.find('(') {
            return s[..idx].trim().to_string();
        }
        for sep in ['<', '>', '='] {
            if let Some(idx) = s.find(sep) {
                return s[..idx].trim().to_string();
            }
        }
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_version_constraints() {
        let s = DependencySanitizer::new();
        let out = s.sanitize(vec!["libssl-dev (>= 1.1)".into(), "libc6:amd64".into()]);
        assert_eq!(out, vec!["openssl".to_string(), "glibc".to_string()]);
    }

    #[test]
    fn keeps_unknown_deps_strictly() {
        let s = DependencySanitizer::new();
        let out = s.sanitize(vec!["some-exotic-lib (>= 2)".into()]);
        assert_eq!(out, vec!["some-exotic-lib".to_string()]);
    }

    #[test]
    fn resolves_alternatives() {
        let s = DependencySanitizer::new();
        let out = s.sanitize(vec!["libssl1.1 | openssl-1.1 | libssl-dev".into()]);
        assert_eq!(out, vec!["openssl".to_string()]);
    }
}