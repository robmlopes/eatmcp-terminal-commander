use eatmcp::web::{format_results, parse_results};

const HTML: &str = r#"
<div class="result"><h2><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2Fpt%2Dbr%3Fa%3D1&amp;rut=abc">Rust <b>Language</b></a></h2>
<a class="result__snippet" href="x">Uma linguagem &amp; r&#x27;apida</a></div>
<div class="result"><a class="result__a" href="https://example.com/">Example</a>
<a class="result__snippet" href="x">Second</a></div>
"#;

#[test]
fn parses_results() {
    let r = parse_results(HTML, 10);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].title, "Rust Language");
    assert_eq!(r[0].url, "https://www.rust-lang.org/pt-br?a=1");
    assert_eq!(r[0].snippet, "Uma linguagem & r'apida");
    assert_eq!(r[1].url, "https://example.com/");
}

#[test]
fn respects_max_and_empty() {
    assert_eq!(parse_results(HTML, 1).len(), 1);
    assert!(format_results("x", &[]).contains("No results"));
}
