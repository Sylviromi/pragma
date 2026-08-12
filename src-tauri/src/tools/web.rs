use std::process::Command;

/// Search the web via DuckDuckGo Lite (free, no key).
/// Parses the simple HTML table that DDG Lite returns.
pub fn web_search(input: &serde_json::Value) -> Result<String, String> {
    let query = input["query"].as_str().ok_or("missing query")?;
    let num = input["num_results"].as_u64().unwrap_or(5).min(10) as usize;

    // URL-encode the query
    let encoded: String = query
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect();

    let url = format!("https://lite.duckduckgo.com/lite/?q={encoded}");
    let out = Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "15",
            "-H",
            "User-Agent: Mozilla/5.0 (compatible; Lynx/2.8)",
            "-H",
            "Accept-Language: en-US,en;q=0.9",
            &url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let html = String::from_utf8_lossy(&out.stdout);
    let results = parse_ddg_lite(&html, num);
    if results.is_empty() {
        Ok("(no results found)".into())
    } else {
        Ok(results.join("\n\n"))
    }
}

/// Parse DuckDuckGo Lite HTML into (title, url, snippet) triples.
///
/// Real DDG Lite markup (results area):
/// ```html
/// <a rel="nofollow" href="//duckduckgo.com/l/?uddg=...&amp;rut=..." class='result-link'>Title</a>
/// <td class='result-snippet'>text here</td>
/// ```
/// Attribute quoting may be single or double; href is a DDG redirect whose real
/// target lives in the percent-encoded `uddg` query parameter.
fn parse_ddg_lite(html: &str, limit: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while results.len() < limit {
        // Locate the next result link attribute position
        let attr_start = match html[pos..].find("result-link") {
            Some(i) => pos + i,
            None => break,
        };
        // Back up to the opening '<' of its enclosing tag
        let Some(tag_start) = html[..=attr_start].rfind('<') else {
            pos = attr_start + 1;
            continue;
        };
        let Some(rel_end) = html[tag_start..].find('>') else {
            break;
        };
        let rel_end = tag_start + rel_end + 1;
        let tag = &html[tag_start..rel_end];

        // Only look at <a> tags
        if !tag.starts_with("<a") {
            pos = rel_end;
            continue;
        }

        let href = extract_attr(tag, "href").unwrap_or_default();
        if href.is_empty() {
            pos = rel_end;
            continue;
        }

        // Title = text between the closing '>' of this tag and </a>
        let title = {
            let inner_start = tag_start + tag[..].find('>').map_or(0, |i| i + 1);
            let inner_end = html[inner_start..]
                .find("</a>")
                .map_or(inner_start, |i| inner_start + i);
            strip_tags(&html[inner_start..inner_end]).trim().to_string()
        };

        // Snippet: next `result-snippet` cell after this result
        let snippet = match html[rel_end..].find("result-snippet") {
            Some(i) => {
                let s_start = rel_end + i;
                let td_end = html[s_start..]
                    .find('>')
                    .map_or(s_start, |j| s_start + j + 1);
                let snip_close_start = html[td_end..].find("</td>").map_or(td_end, |j| td_end + j);
                strip_tags(&html[td_end..snip_close_start])
                    .trim()
                    .to_string()
            }
            None => String::new(),
        };

        if !href.is_empty() && !title.is_empty() {
            let decoded_href = decode_ddg_href(&href);
            results.push(format!("{title}\n{decoded_href}\n{snippet}"));
        }
        pos = rel_end;
    }
    results
}

/// Turn a DDG Lite redirect link (`//duckduckgo.com/l/?uddg=<encoded>&rust=...`) into a
/// human-readable absolute URL. Falls back to the raw href when no `uddg` is present.
fn decode_ddg_href(href: &str) -> String {
    if let Some(qs) = href.split('?').nth(1) {
        for param in qs.split('&') {
            let mut kv = param.splitn(2, '=');
            if kv.next() == Some("uddg") {
                if let Some(val) = kv.next() {
                    let decoded = percent_decode(val);
                    if decoded.starts_with("//") {
                        return format!("https:{decoded}");
                    }
                    if decoded.starts_with("http") {
                        return decoded;
                    }
                }
            }
        }
    }
    href.to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

fn extract_attr(html: &str, attr: &str) -> Option<String> {
    // match attr="..." or attr='...'
    let mut search_from = 0;
    while let Some(rel) = html[search_from..].find(attr) {
        let start = search_from + rel;
        let after = start + attr.len();
        if let Some(q) = html[after..].chars().next() {
            if q == '=' {
                let quote = html[after + 1..].chars().next().unwrap_or('"');
                if quote == '"' || quote == '\'' {
                    let val_start = after + 2;
                    let end = html[val_start..].find(quote)? + val_start;
                    return Some(html[val_start..end].to_string());
                }
            }
        }
        search_from = after;
    }
    None
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode common HTML entities
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

/// Fetch a URL and return its text content with HTML stripped.
pub fn fetch_url(input: &serde_json::Value) -> Result<String, String> {
    let url = input["url"].as_str().ok_or("missing url")?;
    let max_chars = input["max_chars"].as_u64().unwrap_or(8000) as usize;

    let out = Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "20",
            "-H",
            "User-Agent: Mozilla/5.0 (compatible; Lynx/2.8)",
            url,
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }

    let html = String::from_utf8_lossy(&out.stdout);
    let text = strip_tags(&html);
    // Collapse whitespace runs
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(if collapsed.len() > max_chars {
        format!("{}…", &collapsed[..max_chars])
    } else {
        collapsed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_tags() {
        assert_eq!(strip_tags("<b>hello</b> &amp; world"), "hello & world");
    }

    #[test]
    fn test_parse_ddg_lite_empty() {
        // Should not panic on empty/unexpected HTML
        let results = parse_ddg_lite("", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_ddg_lite_sample_single_quotes() {
        let html = "<a rel=\"nofollow\" href=\"//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2F&amp;rut=abc\" class='result-link'>Example Site</a>
                    <td class='result-snippet'>A <b>sample</b> snippet</td>";
        let results = parse_ddg_lite(html, 5);
        assert_eq!(results.len(), 1, "expected 1 result, got {results:?}");
        assert!(results[0].contains("https://example.com/"));
        assert!(results[0].contains("sample snippet"));
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(
            percent_decode("https%3A%2F%2Frust-lang.org%2F"),
            "https://rust-lang.org/"
        );
    }
}
#[cfg(test)]
mod live_tests {
    // Manual live validation — run with: cargo test tools::web::live_tests -- --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_web_search() {
        let r = super::web_search(&serde_json::json!({"query": "tauri v2 rust", "num_results": 3}))
            .unwrap();
        println!("{r}");
        assert!(r.contains("Tauri"));
    }
}
