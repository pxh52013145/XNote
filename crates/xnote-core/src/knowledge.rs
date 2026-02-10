use crate::note_meta::normalize_note_id;
use crate::paths::normalize_vault_rel_path;
use crate::vault::{NoteEntry, Vault};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOptions {
    pub max_files_with_matches: usize,
    pub max_match_rows: usize,
    pub max_preview_matches_per_file: usize,
    pub max_matches_to_count_per_file: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_files_with_matches: 30,
            max_match_rows: 200,
            max_preview_matches_per_file: 3,
            max_matches_to_count_per_file: 50,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchPreviewMatch {
    pub line: usize,
    pub preview: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub path: String,
    pub match_count: usize,
    pub previews: Vec<SearchPreviewMatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchOutcome {
    pub query: String,
    pub elapsed_ms: u128,
    pub hits: Vec<SearchHit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteMetadata {
    pub note_id: Option<String>,
    pub title: String,
    pub frontmatter: HashMap<String, String>,
    pub aliases: Vec<String>,
    pub links: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteSummary {
    pub path: String,
    pub note_id: Option<String>,
    pub title: String,
    pub aliases: Vec<String>,
    pub links: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
struct IndexedNote {
    path: String,
    path_lower: String,
    note_id: Option<String>,
    note_id_lower: Option<String>,
    title: String,
    title_lower: String,
    aliases: Vec<String>,
    aliases_lower: Vec<String>,
    tags: Vec<String>,
    tags_lower: Vec<String>,
    links: Vec<String>,
    links_lower: Vec<String>,
    token_set: HashSet<String>,
}

#[derive(Clone, Debug, Default)]
pub struct KnowledgeIndex {
    notes: HashMap<String, IndexedNote>,
    inverted: HashMap<String, HashSet<String>>,
    note_id_to_path: HashMap<String, String>,
}

impl KnowledgeIndex {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn all_paths_sorted(&self) -> Vec<String> {
        let mut out = self.notes.keys().cloned().collect::<Vec<_>>();
        out.sort();
        out
    }

    pub fn note_summary(&self, note_path: &str) -> Option<NoteSummary> {
        let path = normalize_vault_rel_path(note_path).ok()?;
        let note = self.notes.get(&path)?;
        Some(NoteSummary {
            path: note.path.clone(),
            note_id: note.note_id.clone(),
            title: note.title.clone(),
            aliases: note.aliases.clone(),
            links: note.links.clone(),
            tags: note.tags.clone(),
        })
    }

    pub fn resolve_link_target(&self, raw_link: &str) -> Option<String> {
        let query = normalize_note_link_target(raw_link)?;
        if query.is_empty() {
            return None;
        }

        let query_lower = query.to_lowercase();
        if let Some(note_id) = query_lower.strip_prefix("id:") {
            if let Some(path) = self.note_id_to_path.get(note_id.trim()) {
                return Some(path.clone());
            }
        }
        if let Some(path) = self.note_id_to_path.get(query_lower.trim()) {
            return Some(path.clone());
        }

        let mut candidates = vec![query_lower.clone()];
        if !query_lower.ends_with(".md") {
            candidates.push(format!("{query_lower}.md"));
        }
        if let Some(file_name) = query_lower.rsplit('/').next() {
            candidates.push(file_name.to_string());
            if let Some(stem) = file_name.strip_suffix(".md") {
                candidates.push(stem.to_string());
            }
        }

        for candidate in candidates {
            if let Ok(path) = normalize_vault_rel_path(&candidate) {
                if self.notes.contains_key(&path) {
                    return Some(path);
                }
            }

            if let Some((path, _note)) = self.notes.iter().find(|(_path, note)| {
                note.path_lower == candidate
                    || note.path_lower.ends_with(&format!("/{candidate}"))
                    || note.title_lower == candidate
                    || note.aliases_lower.iter().any(|alias| alias == &candidate)
                    || note
                        .path_lower
                        .rsplit_once('/')
                        .map(|(_, file)| {
                            file == candidate || file.trim_end_matches(".md") == candidate
                        })
                        .unwrap_or(false)
            }) {
                return Some(path.clone());
            }
        }

        None
    }

    pub fn backlinks_for(&self, note_path: &str, max_items: usize) -> Vec<String> {
        let Ok(path) = normalize_vault_rel_path(note_path) else {
            return Vec::new();
        };
        let Some(target) = self.notes.get(&path) else {
            return Vec::new();
        };

        let file_name = target
            .path_lower
            .rsplit_once('/')
            .map(|(_, file)| file.to_string())
            .unwrap_or_else(|| target.path_lower.clone());
        let stem = file_name.trim_end_matches(".md").to_string();
        let mut targets = HashSet::new();
        targets.insert(target.path_lower.clone());
        targets.insert(file_name);
        targets.insert(stem);
        targets.insert(target.title_lower.clone());
        if let Some(note_id) = target.note_id_lower.as_ref() {
            targets.insert(note_id.clone());
            targets.insert(format!("id:{note_id}"));
        }
        for alias in &target.aliases_lower {
            targets.insert(alias.clone());
        }

        let mut out = Vec::new();
        for note in self.notes.values() {
            if note.path == target.path {
                continue;
            }
            let hit = note.links_lower.iter().any(|link| {
                let link = link.trim();
                targets.contains(link)
                    || targets.contains(link.trim_end_matches(".md"))
                    || targets.contains(link.rsplit('/').next().unwrap_or(link))
            });

            if hit {
                out.push(note.path.clone());
                if out.len() >= max_items.max(1) {
                    break;
                }
            }
        }

        out.sort();
        out
    }

    pub fn build_from_entries(vault: &Vault, entries: &[NoteEntry]) -> Result<Self> {
        let mut index = Self::default();
        for entry in entries {
            let _ = index.upsert_note(vault, &entry.path);
        }
        Ok(index)
    }

    pub fn rebuild_from_vault(vault: &Vault) -> Result<Self> {
        let entries = vault.fast_scan_notes()?;
        Self::build_from_entries(vault, &entries)
    }

    pub fn remove_note(&mut self, note_path: &str) {
        let Ok(path) = normalize_vault_rel_path(note_path) else {
            return;
        };

        if let Some(existing) = self.notes.remove(&path) {
            if let Some(note_id) = existing.note_id_lower {
                self.note_id_to_path.remove(&note_id);
            }
            for token in existing.token_set {
                if let Some(paths) = self.inverted.get_mut(&token) {
                    paths.remove(&path);
                    if paths.is_empty() {
                        self.inverted.remove(&token);
                    }
                }
            }
        }
    }

    pub fn upsert_note(&mut self, vault: &Vault, note_path: &str) -> Result<()> {
        let path = normalize_vault_rel_path(note_path)?;
        let content = vault.read_note(&path)?;
        self.remove_note(&path);

        let metadata = parse_note_metadata(&content, &path);
        let path_lower = path.to_lowercase();
        let note_id_lower = metadata.note_id.as_ref().map(|value| value.to_lowercase());
        let title_lower = metadata.title.to_lowercase();
        let aliases_lower = metadata
            .aliases
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        let tags_lower = metadata
            .tags
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        let links_lower = metadata
            .links
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();

        let mut token_set = HashSet::new();
        token_set.extend(tokenize(&path_lower));
        if let Some(note_id) = note_id_lower.as_ref() {
            token_set.extend(tokenize(note_id));
        }
        token_set.extend(tokenize(&title_lower));
        for alias in &aliases_lower {
            token_set.extend(tokenize(alias));
        }
        for tag in &tags_lower {
            token_set.extend(tokenize(tag));
        }
        for link in &links_lower {
            token_set.extend(tokenize(link));
        }
        for value in metadata.frontmatter.values() {
            token_set.extend(tokenize(&value.to_lowercase()));
        }
        for line in content.lines() {
            token_set.extend(tokenize(&line.to_lowercase()));
        }

        for token in &token_set {
            self.inverted
                .entry(token.clone())
                .or_default()
                .insert(path.clone());
        }

        self.notes.insert(
            path.clone(),
            IndexedNote {
                path: path.clone(),
                path_lower,
                note_id: metadata.note_id.clone(),
                note_id_lower: note_id_lower.clone(),
                title: metadata.title.clone(),
                title_lower,
                aliases: metadata.aliases.clone(),
                aliases_lower,
                tags: metadata.tags.clone(),
                tags_lower,
                links: metadata.links.clone(),
                links_lower,
                token_set,
            },
        );

        if let Some(note_id) = note_id_lower {
            self.note_id_to_path.insert(note_id, path);
        }

        Ok(())
    }

    pub fn search(&self, vault: &Vault, query: &str, options: SearchOptions) -> SearchOutcome {
        let started_at = Instant::now();
        let query = query.trim();
        if query.is_empty() {
            return SearchOutcome {
                query: String::new(),
                elapsed_ms: 0,
                hits: Vec::new(),
            };
        }

        let query_lower = query.to_lowercase();
        let query_tokens = tokenize(&query_lower);
        let candidate_paths = self.collect_candidates(&query_lower, &query_tokens);

        let mut ranked = candidate_paths
            .into_iter()
            .filter_map(|path| {
                self.notes.get(&path).map(|note| {
                    (
                        score_note_for_query(note, &query_lower, &query_tokens),
                        note.path.clone(),
                    )
                })
            })
            .filter(|(score, _)| *score > 0)
            .collect::<Vec<_>>();

        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

        let mut hits = Vec::new();
        let mut files_with_matches = 0usize;
        let mut rows = 0usize;

        for (_, path) in ranked {
            if files_with_matches >= options.max_files_with_matches
                || rows >= options.max_match_rows
            {
                break;
            }

            let content = match vault.read_note(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut previews = Vec::new();
            let mut match_count = 0usize;

            for (line_ix, line) in content.lines().enumerate() {
                if match_count >= options.max_matches_to_count_per_file {
                    break;
                }
                if !line.to_lowercase().contains(&query_lower) {
                    continue;
                }

                match_count += 1;
                if previews.len() < options.max_preview_matches_per_file
                    && rows < options.max_match_rows
                {
                    previews.push(SearchPreviewMatch {
                        line: line_ix + 1,
                        preview: line.trim().to_string(),
                    });
                    rows += 1;
                }
            }

            if match_count == 0 {
                if self.notes.get(&path).is_some_and(|n| {
                    n.path_lower.contains(&query_lower) || n.title_lower.contains(&query_lower)
                }) {
                    match_count = 1;
                } else {
                    continue;
                }
            }

            hits.push(SearchHit {
                path,
                match_count,
                previews,
            });
            files_with_matches += 1;
        }

        SearchOutcome {
            query: query.to_string(),
            elapsed_ms: started_at.elapsed().as_millis(),
            hits,
        }
    }

    fn collect_candidates(&self, query_lower: &str, query_tokens: &[String]) -> Vec<String> {
        if query_tokens.is_empty() {
            return self
                .notes
                .values()
                .filter(|note| {
                    note.path_lower.contains(query_lower) || note.title_lower.contains(query_lower)
                })
                .map(|note| note.path.clone())
                .collect();
        }

        let mut sets = query_tokens
            .iter()
            .filter_map(|token| self.inverted.get(token).cloned())
            .collect::<Vec<_>>();

        if sets.is_empty() {
            return self
                .notes
                .values()
                .filter(|note| {
                    note.path_lower.contains(query_lower)
                        || note.title_lower.contains(query_lower)
                        || quick_open_fallback_match(note, query_lower)
                })
                .map(|note| note.path.clone())
                .collect();
        }

        sets.sort_by_key(|s| s.len());
        let mut out = sets.remove(0);
        for set in sets {
            out = out.intersection(&set).cloned().collect();
            if out.is_empty() {
                break;
            }
        }

        out.into_iter().collect()
    }

    pub fn quick_open_paths(&self, query: &str, max_results: usize) -> Vec<String> {
        if max_results == 0 {
            return Vec::new();
        }

        let query = query.trim();
        if query.is_empty() {
            return self
                .all_paths_sorted()
                .into_iter()
                .take(max_results)
                .collect();
        }

        let query_lower = query.to_lowercase();
        let query_tokens = tokenize(&query_lower);
        let mut candidates = self.collect_candidates(&query_lower, &query_tokens);

        let expansion_limit = (max_results.saturating_mul(16)).clamp(256, 4_096);
        if candidates.len() < expansion_limit {
            let mut seen = candidates.iter().cloned().collect::<HashSet<_>>();
            for note in self.notes.values() {
                if seen.contains(&note.path) {
                    continue;
                }
                if !quick_open_fallback_match(note, &query_lower) {
                    continue;
                }

                seen.insert(note.path.clone());
                candidates.push(note.path.clone());
                if candidates.len() >= expansion_limit {
                    break;
                }
            }
        }

        let mut ranked = candidates
            .into_iter()
            .filter_map(|path| {
                self.notes.get(&path).map(|note| {
                    (
                        score_note_for_query(note, &query_lower, &query_tokens),
                        note.path.clone(),
                    )
                })
            })
            .filter(|(score, _)| *score > 0)
            .collect::<Vec<_>>();

        ranked.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.len().cmp(&b.1.len()))
                .then_with(|| a.1.cmp(&b.1))
        });
        ranked
            .into_iter()
            .take(max_results)
            .map(|(_, path)| path)
            .collect()
    }
}

fn score_note_for_query(note: &IndexedNote, query_lower: &str, query_tokens: &[String]) -> usize {
    let mut score = 0usize;
    let file_name = note
        .path_lower
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(note.path_lower.as_str());
    let file_stem = file_name.trim_end_matches(".md");

    if note.title_lower == query_lower {
        score += 220;
    }
    if note.note_id_lower.as_deref() == Some(query_lower) {
        score += 240;
    }
    if note.path_lower == query_lower {
        score += 180;
    }
    if note.title_lower.starts_with(query_lower) {
        score += 130;
    }
    if note.path_lower.starts_with(query_lower) {
        score += 110;
    }
    if note.title_lower.contains(query_lower) {
        score += 70;
    }
    if note.path_lower.contains(query_lower) {
        score += 50;
    }

    if file_stem == query_lower {
        score += 260;
    }
    if file_name == query_lower
        || (!query_lower.ends_with(".md")
            && file_name.starts_with(query_lower)
            && file_name.ends_with(".md")
            && file_name.len() == query_lower.len() + 3)
    {
        score += 180;
    }
    if file_stem.starts_with(query_lower) {
        score += 160;
    }
    if file_name.starts_with(query_lower) {
        score += 120;
    }
    if file_stem.contains(query_lower) {
        score += 90;
    }

    if let Some(fuzzy) = subsequence_score(file_stem, query_lower) {
        score += fuzzy.saturating_mul(6);
    }
    if let Some(fuzzy) = subsequence_score(&note.title_lower, query_lower) {
        score += fuzzy.saturating_mul(3);
    }
    if let Some(fuzzy) = subsequence_score(&note.path_lower, query_lower) {
        score += fuzzy;
    }

    for tag in &note.tags_lower {
        if tag == query_lower {
            score += 40;
        } else if tag.contains(query_lower) {
            score += 24;
        }
    }

    for link in &note.links_lower {
        if link == query_lower {
            score += 24;
        } else if link.contains(query_lower) {
            score += 12;
        }
    }

    for token in query_tokens {
        if note.token_set.contains(token) {
            score += 8;
        }
    }

    if let Some(note_id) = note.note_id_lower.as_deref() {
        if note_id.starts_with(query_lower) {
            score += 36;
        } else if note_id.contains(query_lower) {
            score += 18;
        }
    }

    score
}

fn quick_open_fallback_match(note: &IndexedNote, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }

    if note.path_lower.contains(query_lower) || note.title_lower.contains(query_lower) {
        return true;
    }

    let file_name = note
        .path_lower
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(note.path_lower.as_str());
    let file_stem = file_name.trim_end_matches(".md");

    let query_len = query_lower.chars().count();
    if query_len <= 1 {
        return false;
    }

    subsequence_score(file_stem, query_lower).is_some()
        || (query_len >= 3 && subsequence_score(&note.title_lower, query_lower).is_some())
        || (query_len >= 4 && subsequence_score(&note.path_lower, query_lower).is_some())
}

fn subsequence_score(haystack: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }

    let mut score = 0usize;
    let mut search_start = 0usize;
    let mut first_match = None;
    let mut last_match_end = 0usize;
    let mut prev_match = None;

    for qch in query.chars() {
        let found = haystack[search_start..]
            .char_indices()
            .find(|(_, hch)| *hch == qch)
            .map(|(rel_ix, hch)| (search_start + rel_ix, hch.len_utf8()));
        let (match_ix, match_len) = found?;

        if first_match.is_none() {
            first_match = Some(match_ix);
            score = score.saturating_add(12);
        }

        if let Some(prev_ix) = prev_match {
            let gap_chars = haystack[prev_ix..match_ix]
                .chars()
                .count()
                .saturating_sub(1);
            if gap_chars == 0 {
                score = score.saturating_add(16);
            } else if gap_chars <= 2 {
                score = score.saturating_add(9);
            } else if gap_chars <= 5 {
                score = score.saturating_add(4);
            } else {
                score = score.saturating_add(1);
            }
        }

        if is_word_boundary_at(haystack, match_ix) {
            score = score.saturating_add(7);
        }

        prev_match = Some(match_ix);
        last_match_end = match_ix + match_len;
        search_start = last_match_end;
    }

    let first_match = first_match?;
    let query_len = query.chars().count();
    let span_len = haystack[first_match..last_match_end].chars().count();
    if span_len == query_len {
        score = score.saturating_add(20);
    } else if span_len <= query_len + 2 {
        score = score.saturating_add(10);
    } else {
        score = score.saturating_add(2);
    }

    score = score.saturating_add(query_len.saturating_mul(4));
    Some(score)
}

fn is_word_boundary_at(haystack: &str, index: usize) -> bool {
    if index == 0 {
        return true;
    }

    haystack[..index]
        .chars()
        .next_back()
        .map(|ch| !ch.is_alphanumeric())
        .unwrap_or(true)
}

pub fn parse_note_metadata(content: &str, fallback_path: &str) -> NoteMetadata {
    let title = extract_title(content).unwrap_or_else(|| file_name_from_path(fallback_path));
    let frontmatter = extract_frontmatter(content);
    let note_id = extract_note_id(&frontmatter);
    let aliases = extract_aliases(&frontmatter, &title);
    let mut links = extract_wikilinks(content);
    links.extend(extract_markdown_links(content));
    links = dedup_links_preserve_order(links);
    let tags = extract_tags(content);

    NoteMetadata {
        note_id,
        title,
        frontmatter,
        aliases,
        links,
        tags,
    }
}

fn frontmatter_value<'a>(frontmatter: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    frontmatter.get(key).map(String::as_str).or_else(|| {
        frontmatter
            .iter()
            .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    })
}

fn extract_note_id(frontmatter: &HashMap<String, String>) -> Option<String> {
    let raw = frontmatter_value(frontmatter, "id")?.trim();
    normalize_note_id(raw).ok()
}

fn extract_aliases(frontmatter: &HashMap<String, String>, title: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(raw) = frontmatter_value(frontmatter, "aliases")
        .or_else(|| frontmatter_value(frontmatter, "alias"))
    {
        out.extend(parse_alias_values(raw));
    }

    let title_trimmed = title.trim();
    out.retain(|alias| {
        let alias_trimmed = alias.trim();
        !alias_trimmed.is_empty() && !alias_trimmed.eq_ignore_ascii_case(title_trimmed)
    });
    dedup_links_preserve_order(out)
}

fn parse_alias_values(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let content = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);

    content
        .split(',')
        .map(|part| {
            part.trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|alias| !alias.is_empty())
        .collect()
}

fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

fn extract_frontmatter(content: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return out;
    }

    let mut list_key: Option<String> = None;
    let mut list_values: Vec<String> = Vec::new();

    for raw_line in lines {
        let line = raw_line.trim();
        if line == "---" {
            flush_frontmatter_list(&mut out, &mut list_key, &mut list_values);
            break;
        }

        if list_key.is_some() {
            if line.starts_with("- ") {
                let value = line.trim_start_matches('-').trim();
                if !value.is_empty() {
                    list_values.push(value.to_string());
                }
                continue;
            }

            if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') {
                flush_frontmatter_list(&mut out, &mut list_key, &mut list_values);
            } else {
                continue;
            }
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() {
            if value.is_empty() {
                list_key = Some(key.to_string());
                list_values.clear();
            } else {
                out.insert(key.to_string(), value.to_string());
            }
        }
    }

    flush_frontmatter_list(&mut out, &mut list_key, &mut list_values);

    out
}

fn flush_frontmatter_list(
    out: &mut HashMap<String, String>,
    list_key: &mut Option<String>,
    list_values: &mut Vec<String>,
) {
    let Some(key) = list_key.take() else {
        return;
    };

    if list_values.is_empty() {
        out.insert(key, String::new());
    } else {
        out.insert(key, format!("[{}]", list_values.join(", ")));
    }
    list_values.clear();
}

fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut remain = content;
    while let Some(start) = remain.find("[[") {
        let after = &remain[start + 2..];
        let Some(end) = after.find("]]") else {
            break;
        };
        let raw = after[..end].trim();
        if let Some(target) = normalize_note_link_target(raw) {
            out.push(target);
        }
        remain = &after[end + 2..];
    }
    out
}

fn extract_markdown_links(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = content.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'[' {
            let Some(close_bracket_rel) = content[i + 1..].find(']') else {
                i += 1;
                continue;
            };
            let close_bracket = i + 1 + close_bracket_rel;

            if close_bracket + 1 >= bytes.len() || bytes[close_bracket + 1] != b'(' {
                i = close_bracket.saturating_add(1);
                continue;
            }

            let Some(close_paren_rel) = content[close_bracket + 2..].find(')') else {
                i = close_bracket.saturating_add(2);
                continue;
            };
            let close_paren = close_bracket + 2 + close_paren_rel;

            if i > 0 && bytes[i - 1] == b'!' {
                i = close_paren.saturating_add(1);
                continue;
            }

            let raw_target = content[close_bracket + 2..close_paren].trim();
            if let Some(target) = normalize_note_link_target(raw_target) {
                out.push(target);
            }

            i = close_paren.saturating_add(1);
            continue;
        }

        i += 1;
    }

    out
}

fn dedup_links_preserve_order(links: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for link in links {
        let key = link.to_lowercase();
        if seen.insert(key) {
            out.push(link);
        }
    }

    out
}

fn normalize_note_link_target(raw: &str) -> Option<String> {
    let trimmed = raw
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_alias = trimmed
        .split_once('|')
        .map(|(target, _)| target)
        .unwrap_or(trimmed)
        .trim();
    if without_alias.is_empty() {
        return None;
    }

    let without_heading = without_alias
        .split_once('#')
        .map(|(target, _)| target)
        .unwrap_or(without_alias)
        .trim();
    if without_heading.is_empty() {
        return None;
    }

    let lower = without_heading.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("xnote://")
    {
        return None;
    }

    Some(without_heading.to_string())
}

fn extract_tags(content: &str) -> Vec<String> {
    let mut tags = HashSet::new();
    for token in content.split_whitespace() {
        let Some(rest) = token.strip_prefix('#') else {
            continue;
        };
        let clean = rest
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .to_string();
        if !clean.is_empty() {
            tags.insert(clean);
        }
    }
    let mut out = tags.into_iter().collect::<Vec<_>>();
    out.sort();
    out
}

fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn file_name_from_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_metadata_extracts_title_frontmatter_links_and_tags() {
        let content = r#"---
aliases: Demo
kind: note
---
# My Title
Body with [[Linked/Note]] and #tag_one #tag-two.
"#;

        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(meta.note_id, None);
        assert_eq!(meta.title, "My Title");
        assert_eq!(
            meta.frontmatter.get("aliases").map(String::as_str),
            Some("Demo")
        );
        assert!(meta.aliases.contains(&"Demo".to_string()));
        assert_eq!(meta.links, vec!["Linked/Note"]);
        assert!(meta.tags.contains(&"tag_one".to_string()));
        assert!(meta.tags.contains(&"tag-two".to_string()));
    }

    #[test]
    fn parse_metadata_extracts_aliases_from_frontmatter_lists() {
        let content = r#"---
aliases: ["A", "B Alias", ""]
---
# Title
Body.
"#;
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(meta.note_id, None);
        assert_eq!(meta.aliases, vec!["A".to_string(), "B Alias".to_string()]);
    }

    #[test]
    fn parse_metadata_extracts_normalized_note_id() {
        let content = r#"---
id: 01HABC_123
aliases: ["Guide"]
---
# Title
"#;
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(meta.note_id, Some("01HABC_123".to_string()));
    }

    #[test]
    fn parse_metadata_extracts_aliases_from_frontmatter_block_lists() {
        let content = r#"---
aliases:
  - "Project Guide"
  - Guide
---
# Title
Body.
"#;
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(
            meta.aliases,
            vec!["Project Guide".to_string(), "Guide".to_string()]
        );
    }

    #[test]
    fn parse_metadata_normalizes_wikilink_aliases_and_headings() {
        let content =
            "# T\n[[notes/Alpha|Alias]] and [[notes/Beta#Heading]] and [[notes/Gamma.md#H|Shown]]";
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(
            meta.links,
            vec![
                "notes/Alpha".to_string(),
                "notes/Beta".to_string(),
                "notes/Gamma.md".to_string()
            ]
        );
    }

    #[test]
    fn parse_metadata_extracts_markdown_links_and_skips_external_or_images() {
        let content = "# T\n[Alpha](notes/Alpha.md) [Beta](notes/Beta.md#H) ![img](assets/a.png) [Web](https://example.com)";
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert!(meta.links.contains(&"notes/Alpha.md".to_string()));
        assert!(meta.links.contains(&"notes/Beta.md".to_string()));
        assert!(!meta.links.iter().any(|link| link.contains("http")));
        assert!(!meta.links.iter().any(|link| link.contains("assets/a.png")));
    }

    #[test]
    fn parse_metadata_keeps_spaces_in_link_targets() {
        let content =
            "# T\n[[Project Guide]] and [Guide](notes/Project Guide.md#Summary) and [[Project Guide|Alias]]";
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert!(meta.links.contains(&"Project Guide".to_string()));
        assert!(meta.links.contains(&"notes/Project Guide.md".to_string()));
    }

    #[test]
    fn parse_metadata_dedups_links_preserving_order() {
        let content =
            "# T\n[[notes/Alpha]] [a](notes/Alpha.md) [[notes/Alpha|Again]] [[notes/Beta]]";
        let meta = parse_note_metadata(content, "notes/Fallback.md");
        assert_eq!(
            meta.links,
            vec![
                "notes/Alpha".to_string(),
                "notes/Beta".to_string(),
                "notes/Alpha.md".to_string()
            ]
        );
    }

    #[test]
    fn build_search_and_incremental_update() {
        let temp_dir =
            std::env::temp_dir().join(format!("xnote_core_knowledge_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");
        fs::write(
            temp_dir.join("notes/A.md"),
            "# Alpha\nRust indexing test with #topic and [[notes/B]]",
        )
        .expect("write A");
        fs::write(temp_dir.join("notes/B.md"), "# Beta\nSecond note").expect("write B");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let mut index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let search = index.search(&vault, "rust indexing", SearchOptions::default());
        assert!(search.hits.iter().any(|hit| hit.path == "notes/A.md"));

        fs::write(
            temp_dir.join("notes/B.md"),
            "# Beta\nUpdated with fuzzy token",
        )
        .expect("rewrite B");
        index.upsert_note(&vault, "notes/B.md").expect("upsert B");

        let updated = index.search(&vault, "fuzzy", SearchOptions::default());
        assert!(updated.hits.iter().any(|hit| hit.path == "notes/B.md"));

        index.remove_note("notes/A.md");
        let removed = index.search(&vault, "rust", SearchOptions::default());
        assert!(!removed.hits.iter().any(|hit| hit.path == "notes/A.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn quick_open_prefers_title_and_tag_matches() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_quick_open_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");
        fs::write(
            temp_dir.join("notes/ProjectPlan.md"),
            "# Project Plan\nRoadmap #planning",
        )
        .expect("write ProjectPlan");
        fs::write(temp_dir.join("notes/Daily.md"), "# Daily\nRoutine note").expect("write Daily");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let by_title = index.quick_open_paths("project", 10);
        assert_eq!(
            by_title.first().map(String::as_str),
            Some("notes/ProjectPlan.md")
        );

        let by_tag = index.quick_open_paths("planning", 10);
        assert!(by_tag.iter().any(|p| p == "notes/ProjectPlan.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn quick_open_prefers_filename_stem_over_deeper_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_quick_open_stem_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes/sub")).expect("create test dir");
        fs::write(
            temp_dir.join("notes/Plan.md"),
            "# Generic title\nNo exact title match",
        )
        .expect("write Plan");
        fs::write(
            temp_dir.join("notes/sub/ProjectPlanning.md"),
            "# Planning board\nContains query too",
        )
        .expect("write ProjectPlanning");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let results = index.quick_open_paths("plan", 10);
        assert_eq!(results.first().map(String::as_str), Some("notes/Plan.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn quick_open_supports_subsequence_matching() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_quick_open_subseq_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");
        fs::write(temp_dir.join("notes/ProjectRoadmap.md"), "# Roadmap").expect("write roadmap");
        fs::write(temp_dir.join("notes/Personal.md"), "# Personal").expect("write personal");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let results = index.quick_open_paths("prjrd", 10);
        assert!(results.iter().any(|path| path == "notes/ProjectRoadmap.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn quick_open_tiebreak_prefers_shorter_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_quick_open_tiebreak_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes/a")).expect("create short path dir");
        fs::create_dir_all(temp_dir.join("notes/very/deep/path")).expect("create deep path dir");

        fs::write(temp_dir.join("notes/a/Plan.md"), "# Plan").expect("write short Plan");
        fs::write(temp_dir.join("notes/very/deep/path/Plan.md"), "# Plan")
            .expect("write deep Plan");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        let results = index.quick_open_paths("plan", 10);
        let short_ix = results.iter().position(|path| path == "notes/a/Plan.md");
        let deep_ix = results
            .iter()
            .position(|path| path == "notes/very/deep/path/Plan.md");

        assert!(
            short_ix.is_some() && deep_ix.is_some(),
            "both matches should exist"
        );
        assert!(
            short_ix < deep_ix,
            "shorter path should rank before deeper path on score ties"
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn resolve_link_and_backlinks_are_available() {
        let temp_dir =
            std::env::temp_dir().join(format!("xnote_core_knowledge_links_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");
        fs::write(
            temp_dir.join("notes/Alpha.md"),
            "# Alpha\nBody with [[notes/Beta]]",
        )
        .expect("write Alpha");
        fs::write(temp_dir.join("notes/Beta.md"), "# Beta\n#topic").expect("write Beta");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        assert_eq!(
            index.resolve_link_target("notes/Beta"),
            Some("notes/Beta.md".to_string())
        );
        assert_eq!(
            index.resolve_link_target("Beta"),
            Some("notes/Beta.md".to_string())
        );

        let backlinks = index.backlinks_for("notes/Beta.md", 10);
        assert!(backlinks.iter().any(|path| path == "notes/Alpha.md"));

        let summary = index.note_summary("notes/Beta.md").expect("summary");
        assert_eq!(summary.title, "Beta");
        assert!(summary.aliases.is_empty());
        assert!(summary.tags.iter().any(|tag| tag == "topic"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn resolve_and_backlinks_support_frontmatter_aliases() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_alias_links_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");

        fs::write(
            temp_dir.join("notes/Alpha.md"),
            "# Alpha\nBody with [[Project Guide]]",
        )
        .expect("write Alpha");
        fs::write(
            temp_dir.join("notes/Beta.md"),
            "---\naliases: [\"Project Guide\", \"Guide\"]\n---\n# Beta\n",
        )
        .expect("write Beta");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        assert_eq!(
            index.resolve_link_target("Project Guide"),
            Some("notes/Beta.md".to_string())
        );

        let backlinks = index.backlinks_for("notes/Beta.md", 10);
        assert!(backlinks.iter().any(|path| path == "notes/Alpha.md"));

        let summary = index.note_summary("notes/Beta.md").expect("summary");
        assert!(summary.aliases.iter().any(|alias| alias == "Project Guide"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn resolve_and_backlinks_support_note_id_links() {
        let temp_dir = std::env::temp_dir().join(format!(
            "xnote_core_knowledge_note_id_links_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("notes")).expect("create test dir");

        fs::write(
            temp_dir.join("notes/Alpha.md"),
            "# Alpha\nBody with [[id:01HNOTE_BETA]]",
        )
        .expect("write Alpha");
        fs::write(
            temp_dir.join("notes/Beta.md"),
            "---\nid: 01HNOTE_BETA\n---\n# Beta\n",
        )
        .expect("write Beta");

        let vault = Vault::open(&temp_dir).expect("open vault");
        let entries = vault.fast_scan_notes().expect("scan notes");
        let index = KnowledgeIndex::build_from_entries(&vault, &entries).expect("build index");

        assert_eq!(
            index.resolve_link_target("id:01HNOTE_BETA"),
            Some("notes/Beta.md".to_string())
        );
        assert_eq!(
            index.resolve_link_target("01HNOTE_BETA"),
            Some("notes/Beta.md".to_string())
        );

        let backlinks = index.backlinks_for("notes/Beta.md", 10);
        assert!(backlinks.iter().any(|path| path == "notes/Alpha.md"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
