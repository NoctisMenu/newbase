use egui::{Key, Modifiers, Response, Widget};

use std::cmp::{max, min};
use std::collections::HashMap;

/// Represents a searchable cheat item
#[derive(Debug, Clone)]
pub struct Cheat {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Search result with score and match positions
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub cheat: Cheat,
    pub score: i32,
    pub matched_indices: Vec<usize>,
    pub match_type: MatchType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchType {
    ExactMatch,
    PrefixMatch,
    WordStartMatch,
    SubstringMatch,
    FuzzyMatch,
}

/// Main fuzzy search engine
#[derive(Clone)]
pub struct FuzzySearchEngine {
    cheats: Vec<Cheat>,
    trigram_index: TrigramIndex,
}

impl FuzzySearchEngine {
    pub fn new(cheats: Vec<Cheat>) -> Self {
        let mut engine = Self {
            cheats: cheats.clone(),
            trigram_index: TrigramIndex::new(),
        };

        // Build index for fast searching
        engine.build_index();
        engine
    }

    /// Build trigram index for fast candidate filtering
    fn build_index(&mut self) {
        for (idx, cheat) in self.cheats.iter().enumerate() {
            let searchable = format!(
                "{} {} {} {}",
                cheat.display_name,
                cheat.description,
                cheat.category,
                cheat.tags.join(" ")
            );

            self.trigram_index
                .add_document(idx, &searchable.to_lowercase());
        }
    }

    /// Main search function with configurable options
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        if query.is_empty() {
            return Vec::new();
        }

        let normalized_query = query.to_lowercase().trim().to_string();

        // Get candidate cheats using trigram index
        let candidates = self.trigram_index.get_candidates(&normalized_query);

        let mut results: Vec<SearchResult> = candidates
            .iter()
            .filter_map(|&idx| {
                let cheat = &self.cheats[idx];
                self.score_cheat(cheat, &normalized_query)
            })
            .collect();

        // Sort by score (descending), then by display name for stability
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.cheat.display_name.cmp(&b.cheat.display_name))
        });

        // Return top results
        results.truncate(max_results);
        results
    }

    /// Score a single cheat against the query
    fn score_cheat(&self, cheat: &Cheat, query: &str) -> Option<SearchResult> {
        let name_lower = cheat.display_name.to_lowercase();
        let desc_lower = cheat.description.to_lowercase();
        let category_lower = cheat.category.to_lowercase();

        let mut best_score = 0;
        let mut best_indices = Vec::new();
        let mut best_match_type = MatchType::FuzzyMatch;

        // Check display name (highest weight)
        if let Some((score, indices, match_type)) = Self::match_string(&name_lower, query, 100) {
            if score > best_score {
                best_score = score;
                best_indices = indices;
                best_match_type = match_type;
            }
        }

        // Check description (medium weight)
        if let Some((score, indices, match_type)) = Self::match_string(&desc_lower, query, 60) {
            if score > best_score {
                best_score = score;
                best_indices = indices;
                best_match_type = match_type;
            }
        }

        // Check category (low weight)
        if let Some((score, _, match_type)) = Self::match_string(&category_lower, query, 40) {
            best_score = max(best_score, score);
            if score == best_score {
                best_match_type = match_type;
            }
        }

        // Check tags
        for tag in &cheat.tags {
            let tag_lower = tag.to_lowercase();
            if let Some((score, _, match_type)) = Self::match_string(&tag_lower, query, 50) {
                best_score = max(best_score, score);
                if score == best_score {
                    best_match_type = match_type;
                }
            }
        }

        // Minimum threshold
        if best_score < 20 {
            return None;
        }

        Some(SearchResult {
            cheat: cheat.clone(),
            score: best_score,
            matched_indices: best_indices,
            match_type: best_match_type,
        })
    }

    /// Match a string against query with different algorithms
    fn match_string(text: &str, query: &str, weight: i32) -> Option<(i32, Vec<usize>, MatchType)> {
        // 1. Exact match (highest priority)
        if text == query {
            let indices: Vec<usize> = (0..text.len()).collect();
            return Some((weight, indices, MatchType::ExactMatch));
        }

        // 2. Prefix match
        if text.starts_with(query) {
            let indices: Vec<usize> = (0..query.len()).collect();
            return Some((weight * 90 / 100, indices, MatchType::PrefixMatch));
        }

        // 3. Word start match (matches beginning of words)
        if let Some(indices) = Self::word_start_match(text, query) {
            let score = weight * 80 / 100;
            return Some((score, indices, MatchType::WordStartMatch));
        }

        // 4. Contains match
        if let Some(pos) = text.find(query) {
            let indices: Vec<usize> = (pos..pos + query.len()).collect();
            return Some((weight * 70 / 100, indices, MatchType::SubstringMatch));
        }

        // 5. Fuzzy match (most lenient)
        if let Some((score, indices)) = Self::fuzzy_match(text, query, weight) {
            return Some((score, indices, MatchType::FuzzyMatch));
        }

        None
    }

    /// Match beginning of words (e.g., "gm" matches "God Mode")
    fn word_start_match(text: &str, query: &str) -> Option<Vec<usize>> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let query_chars: Vec<char> = query.chars().collect();

        let mut matched_indices = Vec::new();
        let mut query_idx = 0;
        let mut char_position = 0;

        for word in words {
            if query_idx >= query_chars.len() {
                break;
            }

            let word_chars: Vec<char> = word.chars().collect();
            if !word_chars.is_empty()
                && word_chars[0].to_lowercase().to_string()
                    == query_chars[query_idx].to_lowercase().to_string()
            {
                matched_indices.push(char_position);
                query_idx += 1;
            }

            char_position += word.len() + 1; // +1 for space
        }

        if query_idx == query_chars.len() {
            Some(matched_indices)
        } else {
            None
        }
    }

    /// Fuzzy matching with Levenshtein-based scoring
    fn fuzzy_match(text: &str, query: &str, base_weight: i32) -> Option<(i32, Vec<usize>)> {
        let text_chars: Vec<char> = text.chars().collect();
        let query_chars: Vec<char> = query.chars().collect();

        // Try to find all query characters in order (with gaps allowed)
        let mut matched_indices = Vec::new();
        let mut text_idx = 0;
        let mut query_idx = 0;
        let mut total_gap = 0;

        while query_idx < query_chars.len() && text_idx < text_chars.len() {
            if text_chars[text_idx].to_lowercase().to_string()
                == query_chars[query_idx].to_lowercase().to_string()
            {
                matched_indices.push(text_idx);
                query_idx += 1;
                text_idx += 1;
            } else {
                text_idx += 1;
                total_gap += 1;
            }
        }

        // All query characters must be found
        if query_idx < query_chars.len() {
            // Try Levenshtein distance as fallback
            let distance = levenshtein_distance(text, query);
            if distance <= 2 && distance < query.len() {
                let score = base_weight * 30 / 100 - (distance as i32 * 5);
                return Some((max(score, 20), Vec::new()));
            }
            return None;
        }

        // Calculate score based on match quality
        let gap_penalty = (total_gap as i32).min(20);
        let consecutive_bonus = Self::calculate_consecutive_bonus(&matched_indices);
        let proximity_bonus = Self::calculate_proximity_bonus(&matched_indices);

        let score = (base_weight * 50 / 100) - gap_penalty + consecutive_bonus + proximity_bonus;

        if score < 20 {
            None
        } else {
            Some((score, matched_indices))
        }
    }

    /// Bonus for consecutive character matches
    fn calculate_consecutive_bonus(indices: &[usize]) -> i32 {
        if indices.len() < 2 {
            return 0;
        }

        let mut consecutive_count = 0;
        for i in 1..indices.len() {
            if indices[i] == indices[i - 1] + 1 {
                consecutive_count += 1;
            }
        }

        consecutive_count * 3
    }

    /// Bonus for characters close together
    fn calculate_proximity_bonus(indices: &[usize]) -> i32 {
        if indices.len() < 2 {
            return 0;
        }

        let mut total_distance = 0;
        for i in 1..indices.len() {
            total_distance += indices[i] - indices[i - 1];
        }

        let avg_distance = total_distance / (indices.len() - 1);

        // Lower average distance = higher bonus
        if avg_distance <= 2 {
            10
        } else if avg_distance <= 5 {
            5
        } else {
            0
        }
    }
}

/// Levenshtein distance for typo tolerance
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();

    for (i, c1) in s1_chars.iter().enumerate() {
        for (j, c2) in s2_chars.iter().enumerate() {
            let cost = if c1.to_lowercase().to_string() == c2.to_lowercase().to_string() {
                0
            } else {
                1
            };

            matrix[i + 1][j + 1] = min(
                min(
                    matrix[i][j + 1] + 1, // deletion
                    matrix[i + 1][j] + 1, // insertion
                ),
                matrix[i][j] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

/// Trigram index for fast candidate filtering
#[derive(Clone)]
struct TrigramIndex {
    index: HashMap<String, Vec<usize>>,
}

impl TrigramIndex {
    fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// Add document to index
    fn add_document(&mut self, doc_id: usize, text: &str) {
        let trigrams = Self::generate_trigrams(text);

        for trigram in trigrams {
            self.index
                .entry(trigram)
                .or_insert_with(Vec::new)
                .push(doc_id);
        }
    }

    /// Get candidate documents that share trigrams with query
    fn get_candidates(&self, query: &str) -> Vec<usize> {
        if query.len() < 3 {
            // For short queries, return all documents
            return self.all_document_ids();
        }

        let trigrams = Self::generate_trigrams(query);
        let mut candidate_counts: HashMap<usize, usize> = HashMap::new();

        for trigram in trigrams {
            if let Some(doc_ids) = self.index.get(&trigram) {
                for &doc_id in doc_ids {
                    *candidate_counts.entry(doc_id).or_insert(0) += 1;
                }
            }
        }

        // Return candidates sorted by trigram match count
        let mut candidates: Vec<(usize, usize)> = candidate_counts.into_iter().collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        candidates.into_iter().map(|(id, _)| id).collect()
    }

    /// Generate trigrams from text
    fn generate_trigrams(text: &str) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut trigrams = Vec::new();

        if chars.len() < 3 {
            return trigrams;
        }

        for i in 0..=chars.len() - 3 {
            let trigram: String = chars[i..i + 3].iter().collect();
            trigrams.push(trigram);
        }

        trigrams
    }

    fn all_document_ids(&self) -> Vec<usize> {
        let mut ids: Vec<usize> = self
            .index
            .values()
            .flat_map(|v| v.iter())
            .copied()
            .collect();

        ids.sort();
        ids.dedup();
        ids
    }
}

#[derive(Clone)]
pub struct SearchBar {
    pub search_input: String,
    pub selected_index: Option<usize>,
    pub fuzzy_engine: FuzzySearchEngine,
}
impl SearchBar {
    pub fn new(cheats: Vec<Cheat>) -> Self {
        Self {
            search_input: String::new(),
            selected_index: None,
            fuzzy_engine: FuzzySearchEngine::new(cheats),
        }
    }

    fn update_index(
        &mut self,
        down_pressed: bool,
        up_pressed: bool,
        match_results_count: usize,
        max_suggestions: usize,
    ) {
        self.selected_index = match self.selected_index {
            _ if match_results_count == 0 || max_suggestions == 0 => None,
            // Increment selected index when down is pressed, limit it to the number of matches and max_suggestions
            // Deselect if at last index
            Some(index) if down_pressed => {
                if index + 1 < match_results_count.min(max_suggestions) {
                    Some(index + 1)
                } else {
                    None
                }
            }
            // Decrement selected index if up is pressed. Deselect if at first index
            Some(index) if up_pressed => {
                if index == 0 {
                    None
                } else {
                    Some(index - 1)
                }
            }
            // If nothing is selected and down is pressed, select first item
            None if down_pressed => Some(0),
            // If nothing is selected and up is pressed, select last item
            None if up_pressed => Some(match_results_count.min(max_suggestions) - 1),
            // Do nothing if no keys are pressed
            Some(index) => Some(index),
            None => None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> (Response, Option<Cheat>) {
        let up_pressed =
            ui.input_mut(|input| input.consume_key(Modifiers::default(), Key::ArrowUp));
        let down_pressed =
            ui.input_mut(|input| input.consume_key(Modifiers::default(), Key::ArrowDown));
        let enter_pressed =
            ui.input_mut(|input| input.consume_key(Modifiers::default(), Key::Enter));

        // Style the search bar to match menu theming
        let response = egui::Frame::none()
            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70))
            .rounding(egui::Rounding::same(8.0))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.set_min_width(650.0);
                egui::TextEdit::singleline(&mut self.search_input)
                    .desired_width(650.0)
                    .min_size(egui::vec2(650.0, 0.0))
                    .hint_text(egui::RichText::new("Search settings...").color(egui::Color32::GRAY))
                    .frame(false)
                    .ui(ui)
            })
            .inner;

        let results = self.fuzzy_engine.search(&self.search_input, 5);
        self.update_index(down_pressed, up_pressed, results.len(), 5);
        if response.changed()
            || (self.selected_index.is_some() && self.selected_index.unwrap() >= results.len())
        {
            self.selected_index = None;
        }

        let id = ui.next_auto_id();
        ui.skip_ahead_auto_ids(1);

        let mut clicked_result: Option<Cheat> = None;

        // Check if popup was open in previous frame AND has results
        let popup_was_open = ui.memory(|mem| mem.is_popup_open(id));
        let has_results = !self.search_input.is_empty() && !results.is_empty();

        // Show popup if input is focused and has text, OR if it was already open (to catch clicks)
        let should_show_popup = (response.has_focus() && has_results) || popup_was_open;

        // Render popup area if it should be visible
        if should_show_popup && has_results {
            egui::Area::new(id)
                .order(egui::Order::Foreground)
                .fixed_pos(response.rect.left_bottom() + egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 90))
                        .rounding(egui::Rounding::same(8.0))
                        .inner_margin(8.0)
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 40)))
                        .show(ui, |ui| {
                            ui.set_min_width(response.rect.width());

                            for (i, result) in results.iter().enumerate() {
                                let is_selected = self.selected_index.map_or(false, |x| x == i);

                                // Create a button-like selectable for each result
                                let mut label_response =
                                    ui.selectable_label(is_selected, &result.cheat.display_name);

                                // Display description if available
                                if !result.cheat.description.is_empty() {
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new(&result.cheat.description)
                                            .size(11.0)
                                            .color(egui::Color32::GRAY),
                                    );
                                }

                                // Update selected index based on hover
                                if label_response.hovered() {
                                    self.selected_index = Some(i);
                                }

                                // Handle click
                                if label_response.clicked() {
                                    clicked_result = Some(result.cheat.clone());
                                    label_response.mark_changed();
                                }

                                // Add spacing between results
                                if i < results.len() - 1 {
                                    ui.add_space(4.0);
                                }
                            }
                        });
                });
        }

        // Manage popup open/close state
        if response.has_focus() && has_results {
            ui.memory_mut(|mem| mem.open_popup(id));
        } else if !popup_was_open || clicked_result.is_some() {
            // Close popup if it wasn't open, or if something was clicked
            ui.memory_mut(|mem| mem.close_popup());
        }

        // Handle selection via click
        if let Some(cheat) = clicked_result {
            self.search_input.clear();
            self.selected_index = None;
            ui.memory_mut(|mem| {
                mem.close_popup();
                mem.surrender_focus(response.id);
            });
            return (response, Some(cheat));
        }

        if enter_pressed && self.selected_index.is_some() {
            self.search_input.clear();
            let selected = Some(results[self.selected_index.unwrap()].cheat.clone());
            self.selected_index = None;
            ui.memory_mut(|mem| {
                mem.close_popup();
                mem.surrender_focus(response.id);
            });
            return (response, selected);
        }

        (response, None)
    }
}
