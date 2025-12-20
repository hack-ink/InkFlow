use std::collections::VecDeque;

use crate::stt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenMode {
	Words,
	Chars,
}

#[derive(Clone, Debug)]
pub(crate) struct NToken {
	display: String,
	norm: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MergeState {
	stable: Vec<NToken>,
	recent: VecDeque<Vec<NToken>>,
}

impl MergeState {
	pub(crate) fn reset(&mut self) {
		self.stable.clear();
		self.recent.clear();
	}

	pub(crate) fn apply_candidate(
		&mut self,
		candidate_text: &str,
		mode: TokenMode,
		stable_ticks: usize,
		rollback_threshold: usize,
	) -> String {
		let candidate_tokens = tokenize(candidate_text, mode);
		let stable_lcp = lcp_len(&self.stable, &candidate_tokens);

		if self.stable.len() > stable_lcp.saturating_add(rollback_threshold) {
			self.stable.truncate(stable_lcp);
			self.recent.clear();
		}

		self.recent.push_back(candidate_tokens.clone());
		while self.recent.len() > stable_ticks.max(1) {
			self.recent.pop_front();
		}

		if self.recent.len() == stable_ticks.max(1) {
			let lcp_all = lcp_len_all(&self.recent);
			if lcp_all > self.stable.len() {
				self.stable = candidate_tokens.iter().take(lcp_all).cloned().collect();
			}
		}

		join_stable_and_candidate(&self.stable, &candidate_tokens, mode)
	}
}

pub(crate) fn token_mode_for_language(language: &str, text: &str) -> TokenMode {
	let lang = language.trim().to_lowercase();

	let is_cjk = lang.starts_with("zh") || lang.starts_with("ja") || lang.starts_with("ko");
	if is_cjk {
		return TokenMode::Chars;
	}

	if lang == "auto" {
		return if text.chars().any(|c| c.is_whitespace()) {
			TokenMode::Words
		} else {
			TokenMode::Chars
		};
	}

	TokenMode::Words
}

pub(crate) fn collapse_leading_duplicate_word(text: &str) -> String {
	let mut iter = text.split_whitespace();
	let Some(first) = iter.next() else {
		return String::new();
	};

	let mut tokens = vec![first];
	tokens.extend(iter);

	if tokens.len() < 2 {
		return tokens[0].to_string();
	}

	let first_norm = normalize_word(tokens[0]);
	if first_norm.len() < 4 {
		return text.trim().to_string();
	}

	let mut end_of_run = 1usize;
	while end_of_run < tokens.len() && normalize_word(tokens[end_of_run]) == first_norm {
		end_of_run += 1;
	}

	if end_of_run <= 1 {
		return text.trim().to_string();
	}

	let mut out = String::new();
	out.push_str(tokens[end_of_run.saturating_sub(1)]);
	for token in tokens.into_iter().skip(end_of_run) {
		out.push(' ');
		out.push_str(token);
	}

	out
}

pub(crate) fn choose_segment_provisional_text(
	sherpa_text: &str,
	live_text: &str,
	live_has_window: bool,
) -> String {
	let sherpa_text = sherpa_text.trim();
	let live_text = live_text.trim();

	if sherpa_text.is_empty() && !live_text.is_empty() {
		return live_text.to_string();
	}

	if !live_has_window || live_text.is_empty() {
		return sherpa_text.to_string();
	}

	let mode = if sherpa_text.chars().any(|c| c.is_whitespace())
		|| live_text.chars().any(|c| c.is_whitespace())
	{
		TokenMode::Words
	} else {
		TokenMode::Chars
	};

	let sherpa_tokens = tokenize(sherpa_text, mode);
	let live_tokens = tokenize(live_text, mode);
	let lcp = lcp_len(&sherpa_tokens, &live_tokens);

	if lcp == 0 {
		if mode == TokenMode::Words
			&& window_adds_only_long_leading_words(&sherpa_tokens, &live_tokens)
		{
			return live_text.to_string();
		}
		return sherpa_text.to_string();
	}

	if sherpa_text.starts_with(live_text) {
		return sherpa_text.to_string();
	}

	live_text.to_string()
}

pub(crate) fn should_accept_second_pass_replacement(
	current: &str,
	incoming: &str,
	mode: TokenMode,
) -> bool {
	let current = current.trim();
	let incoming = incoming.trim();

	if current.is_empty() || incoming.is_empty() {
		return true;
	}

	if mode != TokenMode::Words {
		return true;
	}

	let current_words: Vec<&str> = current.split_whitespace().collect();
	let incoming_words: Vec<&str> = incoming.split_whitespace().collect();

	if current_words.len() >= 3 && incoming_words.len() == 1 {
		let Some(last_word) = current_words.last() else {
			return true;
		};

		let incoming_norm = normalize_word(incoming_words[0]);
		let last_norm = normalize_word(last_word);
		if !incoming_norm.is_empty() && incoming_norm == last_norm {
			return false;
		}
	}

	true
}

pub(crate) fn leading_words_compatible(a: &str, b: &str) -> bool {
	let a = normalize_word(a);
	let b = normalize_word(b);

	if a.is_empty() || b.is_empty() {
		return false;
	}
	if a == b {
		return true;
	}

	let min_len = a.len().min(b.len());
	if min_len < 3 {
		return false;
	}

	let common_prefix = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
	common_prefix >= 3
}

pub(crate) fn dedup_tail(
	committed_text: &str,
	tail_text: &str,
	mode: TokenMode,
	overlap_k_words: usize,
	overlap_k_chars: usize,
) -> String {
	let tail_trimmed = tail_text.trim();
	if tail_trimmed.is_empty() {
		return String::new();
	}

	let committed_tokens = tokenize(committed_text, mode);
	let mut tail_tokens = tokenize(tail_trimmed, mode);
	if committed_tokens.is_empty() || tail_tokens.is_empty() {
		return join_tokens(&tail_tokens, mode);
	}

	let k = match mode {
		TokenMode::Words => overlap_k_words.max(1),
		TokenMode::Chars => overlap_k_chars.max(1),
	};

	let committed_suffix_start = committed_tokens.len().saturating_sub(k);
	let tail_prefix_end = tail_tokens.len().min(k);
	let committed_suffix = &committed_tokens[committed_suffix_start..];
	let tail_prefix = &tail_tokens[..tail_prefix_end];

	let max_overlap = committed_suffix.len().min(tail_prefix.len());
	let mut overlap = 0usize;
	for n in (1..=max_overlap).rev() {
		let left = &committed_suffix[committed_suffix.len().saturating_sub(n)..];
		let right = &tail_prefix[..n];
		if left.iter().zip(right.iter()).all(|(a, b)| a.norm == b.norm) {
			overlap = n;
			break;
		}
	}

	if overlap > 0 {
		tail_tokens.drain(..overlap);
	}

	join_tokens(&tail_tokens, mode)
}

pub(crate) fn extract_window_tail_text(
	snapshot: &stt::WindowJobSnapshot,
	committed_end_16k_samples: u64,
	language: &str,
	segments: &[stt::WhisperDecodedSegment],
) -> String {
	let sample_text =
		segments.iter().find(|s| !s.text.trim().is_empty()).map(|s| &*s.text).unwrap_or("");
	let mode = token_mode_for_language(language, sample_text);

	let window_end = snapshot.window_end_16k_samples;
	let window_start = window_end.saturating_sub(snapshot.window_len_16k_samples as u64);

	let cut = committed_end_16k_samples
		.saturating_sub(snapshot.context_len_16k_samples as u64)
		.max(window_start);

	let mut out = String::new();
	for seg in segments {
		let t1_ms = seg.t1_ms;
		let seg_abs_end = window_start.saturating_add(ms_to_samples_16k(t1_ms));
		if seg_abs_end <= cut {
			continue;
		}

		let text = seg.text.trim();
		if text.is_empty() {
			continue;
		}

		match mode {
			TokenMode::Words => {
				if !out.is_empty() {
					out.push(' ');
				}
				out.push_str(text);
			},
			TokenMode::Chars => {
				out.push_str(text);
			},
		}
	}

	out
}

pub(crate) fn ms_to_samples_16k(ms: u64) -> u64 {
	ms.saturating_mul(16_000).saturating_div(1_000)
}

fn tokenize(text: &str, mode: TokenMode) -> Vec<NToken> {
	match mode {
		TokenMode::Words => text
			.split_whitespace()
			.map(|w| NToken { display: w.to_string(), norm: normalize_word(w) })
			.collect(),
		TokenMode::Chars => text
			.chars()
			.map(|c| {
				let display = c.to_string();
				let norm = if c.is_whitespace() {
					" ".to_string()
				} else {
					c.to_lowercase().collect::<String>()
				};
				NToken { display, norm }
			})
			.collect(),
	}
}

fn normalize_word(word: &str) -> String {
	let trimmed = word.trim_matches(|c: char| !c.is_alphanumeric());
	trimmed.to_lowercase()
}

fn window_adds_only_long_leading_words(sherpa: &[NToken], live: &[NToken]) -> bool {
	if sherpa.is_empty() {
		return false;
	}
	if live.len() <= sherpa.len() {
		return false;
	}

	let extra = live.len().saturating_sub(sherpa.len());
	if extra == 0 || extra > 2 {
		return false;
	}

	if !live[extra..].iter().map(|t| &t.norm).eq(sherpa.iter().map(|t| &t.norm)) {
		return false;
	}

	live[..extra].iter().all(|token| token.norm.len() >= 4)
}

fn lcp_len(a: &[NToken], b: &[NToken]) -> usize {
	let mut n = 0usize;
	let min_len = a.len().min(b.len());
	while n < min_len {
		if a[n].norm != b[n].norm {
			break;
		}
		n += 1;
	}
	n
}

fn lcp_len_all(candidates: &VecDeque<Vec<NToken>>) -> usize {
	let Some(first) = candidates.front() else {
		return 0;
	};

	let mut lcp = first.len();
	for c in candidates.iter().skip(1) {
		let min_len = lcp.min(c.len());
		let mut n = 0usize;
		while n < min_len {
			if first[n].norm != c[n].norm {
				break;
			}
			n += 1;
		}
		lcp = lcp.min(n);
		if lcp == 0 {
			break;
		}
	}

	lcp
}

fn join_stable_and_candidate(stable: &[NToken], candidate: &[NToken], mode: TokenMode) -> String {
	let mut out = String::new();
	let stable_len = stable.len().min(candidate.len());

	match mode {
		TokenMode::Words => {
			for token in stable.iter().take(stable_len).chain(candidate.iter().skip(stable_len)) {
				if token.display.trim().is_empty() {
					continue;
				}
				if !out.is_empty() {
					out.push(' ');
				}
				out.push_str(&token.display);
			}
		},
		TokenMode::Chars => {
			for token in stable.iter().take(stable_len).chain(candidate.iter().skip(stable_len)) {
				out.push_str(&token.display);
			}
		},
	}

	out
}

fn join_tokens(tokens: &[NToken], mode: TokenMode) -> String {
	let mut out = String::new();
	match mode {
		TokenMode::Words =>
			for token in tokens {
				if token.display.trim().is_empty() {
					continue;
				}
				if !out.is_empty() {
					out.push(' ');
				}
				out.push_str(&token.display);
			},
		TokenMode::Chars =>
			for token in tokens {
				out.push_str(&token.display);
			},
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn collapse_leading_duplicate_word_removes_one_copy() {
		let input = "walking walking is a great way to unwind";
		let out = collapse_leading_duplicate_word(input);
		assert_eq!(out, "walking is a great way to unwind");
	}

	#[test]
	fn collapse_leading_duplicate_word_keeps_short_words() {
		let input = "had had enough";
		let out = collapse_leading_duplicate_word(input);
		assert_eq!(out, input);
	}

	#[test]
	fn leading_words_compatible_handles_prefix_matching() {
		assert!(leading_words_compatible("Sometimes", "SOME"));
		assert!(!leading_words_compatible("So", "SOME"));
	}

	#[test]
	fn dedup_tail_drops_overlap_words() {
		let committed = "walking";
		let tail = "walking is a great way";
		let out = dedup_tail(committed, tail, TokenMode::Words, 30, 100);
		assert_eq!(out, "is a great way");
	}

	#[test]
	fn choose_segment_provisional_text_allows_long_leading_word_from_window() {
		let sherpa = "I GO ALONE";
		let live = "Usually, I go alone.";
		let out = choose_segment_provisional_text(sherpa, live, true);
		assert_eq!(out, live);
	}

	#[test]
	fn choose_segment_provisional_text_rejects_short_leading_word_from_window() {
		let sherpa = "Sometimes I go";
		let live = "So Sometimes I go";
		let out = choose_segment_provisional_text(sherpa, live, true);
		assert_eq!(out, sherpa);
	}

	#[test]
	fn second_pass_replacement_rejects_single_word_suffix() {
		assert!(!should_accept_second_pass_replacement("I GO ALONE", "alone", TokenMode::Words));
		assert!(should_accept_second_pass_replacement(
			"I GO ALONE",
			"I go alone",
			TokenMode::Words
		));
	}
}
