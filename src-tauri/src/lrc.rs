//! LRC parsing and title cleanup. Pure logic, unit-tested.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Line {
    /// Start time in seconds.
    pub t: f64,
    pub text: String,
}

/// Parse one leading `[mm:ss.xx]` tag. Returns (seconds, remainder of the line).
fn take_time_tag(s: &str) -> Option<(f64, &str)> {
    let s = s.strip_prefix('[')?;
    let end = s.find(']')?;
    let inner = &s[..end];
    let rest = &s[end + 1..];
    let (mm, tail) = inner.split_once(':')?;
    if mm.is_empty() || !mm.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (ss, frac) = match tail.find(|c: char| c == '.' || c == ':') {
        Some(i) => (&tail[..i], &tail[i + 1..]),
        None => (tail, ""),
    };
    if ss.is_empty() || !ss.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mins: f64 = mm.parse().ok()?;
    let secs: f64 = ss.parse().ok()?;
    let fraction = if frac.is_empty() {
        0.0
    } else {
        frac.parse::<f64>().ok()? / 10f64.powi(frac.len() as i32)
    };
    Some((mins * 60.0 + secs + fraction, rest))
}

fn looks_like_time(s: &str) -> bool {
    s.contains(':') && s.bytes().all(|b| b.is_ascii_digit() || b == b':' || b == b'.')
}

/// Remove enhanced-LRC word tags such as `<00:12.30>`.
fn strip_word_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        match rest[start..].find('>') {
            Some(end) if looks_like_time(&rest[start + 1..start + end]) => {
                rest = &rest[start + end + 1..];
            }
            _ => {
                out.push('<');
                rest = &rest[start + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parse synced lyrics. Returns an empty vec when the text has no timestamps.
pub fn parse_lrc(src: &str) -> Vec<Line> {
    let mut offset_s = 0.0;
    let mut out = Vec::new();
    for raw in src.lines() {
        let mut line = raw.trim_start_matches('\u{feff}').trim();
        if let Some(v) = line.strip_prefix("[offset:") {
            if let Some(num) = v.strip_suffix(']') {
                if let Ok(ms) = num.trim().parse::<f64>() {
                    offset_s = ms / 1000.0;
                }
            }
            continue;
        }
        let mut times = Vec::new();
        while let Some((t, rest)) = take_time_tag(line) {
            times.push(t);
            line = rest.trim_start();
        }
        if times.is_empty() {
            continue;
        }
        let text = strip_word_tags(line)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        for t in times {
            out.push(Line {
                t: (t - offset_s).max(0.0),
                text: text.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    out
}

const NOISE_WORDS: [&str; 20] = [
    "feat", "ft", "featuring", "remaster", "remastered", "live", "version", "edit", "mix",
    "official", "video", "audio", "lyric", "lyrics", "explicit", "deluxe", "mono", "stereo",
    "bonus", "radio",
];

fn has_noise_word(s: &str) -> bool {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| NOISE_WORDS.contains(&w))
}

/// Strip "(feat. X)", "[Official Video]", "- 2011 Remaster" etc. so lyric lookups match.
pub fn clean_title(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '(' || c == '[' {
            let close = if c == '(' { ')' } else { ']' };
            if let Some(off) = chars[i + 1..].iter().position(|&x| x == close) {
                let inner: String = chars[i + 1..i + 1 + off].iter().collect();
                if has_noise_word(&inner) {
                    i += off + 2;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    if let Some(idx) = out.find(" - ") {
        if has_noise_word(&out[idx + 3..]) {
            out.truncate(idx);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Artist - Topic" / "ArtistVEVO" (YouTube channel names) -> "Artist"
pub fn clean_artist(artist: &str) -> String {
    let mut a = artist.trim().to_string();
    for suffix in [" - topic", "vevo"] {
        let n = suffix.len();
        if a.len() > n && a.is_char_boundary(a.len() - n) && a[a.len() - n..].eq_ignore_ascii_case(suffix) {
            a.truncate(a.len() - n);
            a = a.trim().to_string();
        }
    }
    a
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

/// 0.0..=1.0: how many of the shorter title's words also appear in the other title.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let (wa, wb) = (words(a), words(b));
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let (short, long) = if wa.len() <= wb.len() { (&wa, &wb) } else { (&wb, &wa) };
    let hits = short.iter().filter(|w| long.contains(*w)).count();
    hits as f64 / short.len() as f64
}

/// "A, B & C" -> "A"
pub fn primary_artist(artist: &str) -> String {
    artist
        .split(|c: char| c == ',' || c == ';' || c == '&' || c == '/')
        .next()
        .unwrap_or(artist)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_and_multi_timestamps() {
        let l = parse_lrc("[ar:Someone]\n[00:12.50] Hello\n[01:02.5][00:30.00] Again <00:30.10>world\n");
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], Line { t: 12.5, text: "Hello".into() });
        assert_eq!(l[1], Line { t: 30.0, text: "Again world".into() });
        assert_eq!(l[2].t, 62.5);
    }

    #[test]
    fn applies_offset_and_keeps_empty_lines() {
        let l = parse_lrc("[offset:500]\n[00:10.00]\n[00:12.000] Hi\n");
        assert_eq!(l[0], Line { t: 9.5, text: "".into() });
        assert_eq!(l[1].t, 11.5);
    }

    #[test]
    fn no_timestamps_gives_empty() {
        assert!(parse_lrc("just some\nplain lyrics").is_empty());
    }

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("Song (feat. X)"), "Song");
        assert_eq!(clean_title("Song - 2011 Remaster"), "Song");
        assert_eq!(clean_title("Song [Official Video]"), "Song");
        assert_eq!(clean_title("Song (Part II)"), "Song (Part II)");
        assert_eq!(clean_title("Alive (Live at Wembley)"), "Alive");
        assert_eq!(clean_title("Alive"), "Alive");
    }

    #[test]
    fn cleans_youtube_artist_names() {
        assert_eq!(clean_artist("Coldplay - Topic"), "Coldplay");
        assert_eq!(clean_artist("AdeleVEVO"), "Adele");
        assert_eq!(clean_artist("Vevo"), "Vevo");
        assert_eq!(clean_artist("Tulus"), "Tulus");
    }

    #[test]
    fn title_similarity_works() {
        assert_eq!(title_similarity("Hati-Hati di Jalan", "hati hati di jalan"), 1.0);
        assert!(title_similarity("Hati-Hati di Jalan", "Hati Hati Di Jalan (Live)") >= 0.99);
        assert!(title_similarity("Home", "Welcome Home") >= 0.99); // short vs long: needs the artist/duration check too
        assert!(title_similarity("Sandiwara Cinta", "Different Song Entirely") < 0.5);
        assert_eq!(title_similarity("", "x"), 0.0);
    }

    #[test]
    fn primary_artist_split() {
        assert_eq!(primary_artist("A, B & C"), "A");
        assert_eq!(primary_artist("Solo"), "Solo");
    }
}
