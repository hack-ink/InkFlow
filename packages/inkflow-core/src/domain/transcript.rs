use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenMode {
	Words,
	Chars,
}

#[derive(Clone, Debug)]
pub struct NToken {
	display: String,
	norm: String,
}

#[derive(Clone, Debug, Default)]
pub struct MergeState {
	stable: Vec<NToken>,
	recent: VecDeque<Vec<NToken>>,
}

impl MergeState {
	pub fn reset(&mut self) {
		self.stable.clear();
		self.recent.clear();
	}

	pub fn apply_candidate(
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

pub fn token_mode_for_language(language: &str, text: &str) -> TokenMode {
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

pub fn collapse_leading_duplicate_word(text: &str) -> String {
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

pub fn choose_segment_provisional_text(
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

pub fn should_accept_second_pass_replacement(
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

pub fn leading_words_compatible(a: &str, b: &str) -> bool {
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

pub fn dedup_tail(
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

pub fn extract_window_tail_text(text: &str, window_len_16k_samples: usize) -> String {
	let tokens = tokenize(text, TokenMode::Words);
	let words = tokens.len();
	if words <= 1 {
		return text.trim().to_string();
	}

	let estimated_tokens = window_len_16k_samples.saturating_mul(16).saturating_div(1000).max(1);
	let take = estimated_tokens.min(words);
	let start = words.saturating_sub(take);
	let tail_tokens = &tokens[start..];
	join_tokens(tail_tokens, TokenMode::Words)
}

pub fn ms_to_samples_16k(ms: u64) -> u64 {
	const SAMPLE_RATE_HZ: u64 = 16_000;
	SAMPLE_RATE_HZ.saturating_mul(ms).saturating_div(1000)
}

fn join_stable_and_candidate(stable: &[NToken], candidate: &[NToken], mode: TokenMode) -> String {
	if stable.is_empty() {
		return join_tokens(candidate, mode);
	}

	let mut merged = Vec::new();
	merged.extend_from_slice(stable);
	merged.extend_from_slice(&candidate[stable.len().min(candidate.len())..]);
	join_tokens(&merged, mode)
}

fn join_tokens(tokens: &[NToken], mode: TokenMode) -> String {
	let mut out = String::new();
	for (idx, token) in tokens.iter().enumerate() {
		if idx > 0 && mode == TokenMode::Words {
			out.push(' ');
		}
		out.push_str(&token.display);
	}
	out
}

fn tokenize(text: &str, mode: TokenMode) -> Vec<NToken> {
	match mode {
		TokenMode::Words => text
			.split_whitespace()
			.map(|token| NToken { display: token.to_string(), norm: normalize_word(token) })
			.collect(),
		TokenMode::Chars => text
			.chars()
			.filter(|c| !c.is_whitespace())
			.map(|c| {
				let display = c.to_string();
				let norm = normalize_word(&display);
				NToken { display, norm }
			})
			.collect(),
	}
}

fn normalize_word(token: &str) -> String {
	let token = token.trim();
	let mut out = String::new();
	for ch in token.chars() {
		if ch.is_alphanumeric() || ch.is_whitespace() {
			out.push(ch.to_ascii_lowercase());
		}
	}
	out
}

fn lcp_len(a: &[NToken], b: &[NToken]) -> usize {
	let mut n = 0usize;
	for (left, right) in a.iter().zip(b.iter()) {
		if left.norm == right.norm {
			n += 1;
		} else {
			break;
		}
	}

	n
}

fn lcp_len_all(candidate_sets: &VecDeque<Vec<NToken>>) -> usize {
	let Some(first) = candidate_sets.front() else {
		return 0;
	};

	let mut lcp_count = first.len();
	for set in candidate_sets.iter().skip(1) {
		lcp_count = lcp_count.min(lcp_len(first, set));
		if lcp_count == 0 {
			break;
		}
	}

	lcp_count
}

fn window_adds_only_long_leading_words(a: &[NToken], b: &[NToken]) -> bool {
	if b.len() < a.len() {
		return false;
	}

	let suffix_start = b.len().saturating_sub(a.len());
	let suffix = &b[suffix_start..];
	if suffix.len() != a.len() {
		return false;
	}

	for (left, right) in a.iter().zip(suffix.iter()) {
		if left.norm != right.norm {
			return false;
		}
	}

	let leading = &b[..suffix_start];
	leading.iter().all(|token| token.norm.len() >= 4)
}
