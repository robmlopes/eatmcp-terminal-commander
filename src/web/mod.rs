//! Busca na web via DuckDuckGo (endpoint HTML, sem chave de API).

use regex::Regex;
use std::time::Duration;

const ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Executa a busca e devolve o texto formatado.
pub async fn web_search(query: &str, max_results: Option<usize>) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("'query' is required and must not be empty".to_string());
    }
    let max = max_results.unwrap_or(10).clamp(1, 30);

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let resp = client
        .post(ENDPOINT)
        .form(&[("q", query)])
        .send()
        .await
        .map_err(|e| format!("Search request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Search engine returned HTTP {}", resp.status()));
    }
    let html = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let results = parse_results(&html, max);
    Ok(format_results(query, &results))
}

pub fn format_results(query: &str, results: &[SearchResult]) -> String {
    if results.is_empty() {
        return format!("No results found for '{}'", query);
    }
    let mut out = format!("Search results for '{}':\n", query);
    for (i, r) in results.iter().enumerate() {
        out.push_str(&format!("\n{}. {}\n   {}\n", i + 1, r.title, r.url));
        if !r.snippet.is_empty() {
            out.push_str(&format!("   {}\n", r.snippet));
        }
    }
    out
}

pub fn parse_results(html: &str, max: usize) -> Vec<SearchResult> {
    let link_re = Regex::new(r#"(?s)<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>"#).unwrap();
    let snip_re = Regex::new(r#"(?s)<a[^>]*class="result__snippet"[^>]*>(.*?)</a>"#).unwrap();

    let links: Vec<_> = link_re.captures_iter(html).collect();
    let snippets: Vec<_> = snip_re.captures_iter(html).collect();

    links
        .iter()
        .enumerate()
        .take(max)
        .map(|(i, c)| SearchResult {
            title: clean_text(&c[2]),
            url: resolve_url(&c[1]),
            snippet: snippets.get(i).map(|s| clean_text(&s[1])).unwrap_or_default(),
        })
        .collect()
}

/// Links do DDG vêm como `//duckduckgo.com/l/?uddg=<url codificada>&rut=...`.
fn resolve_url(href: &str) -> String {
    let href = decode_entities(href);
    if let Some(pos) = href.find("uddg=") {
        let rest = &href[pos + 5..];
        let enc = rest.split('&').next().unwrap_or(rest);
        return percent_decode(enc);
    }
    if href.starts_with("//") {
        return format!("https:{}", href);
    }
    href
}

fn clean_text(s: &str) -> String {
    let tag_re = Regex::new(r"(?s)<[^>]*>").unwrap();
    let no_tags = tag_re.replace_all(s, "");
    decode_entities(&no_tags)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                let hex = b
                    .get(i + 1..i + 3)
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(v) = hex {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
