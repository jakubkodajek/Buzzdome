//! The internet for Studio's AI assistant (docs/ai-assistants.md): when
//! the quizmaster asks for "a quiz about this company, from its web site",
//! the model says what to look up (`local_model::research_plan`), the app
//! searches for it and reads the pages, and the model writes the questions
//! from what they say. Pure: the app fetches the pages; here is how a
//! search is asked and read (`search_address`, `read_results`), what is read
//! of a page (`read_page`), which of its links to follow (`links_to_follow`)
//! and the text the model gets (`research_text`).
//!
//! The search is DuckDuckGo's page without scripts, which takes no key or
//! account (as Open WebUI's DuckDuckGo search does,
//! https://docs.openwebui.com/features/chat-conversations/web-search/providers/ddgs).
//! A page is read as its text: no scripts run, so a site that is drawn by
//! scripts alone gives little.

use serde::Serialize;
use ts_rs::TS;
pub use url::Url;

/// Most pages read for one request, the sites named and the results found.
pub const MAX_PAGES: usize = 6;
/// Pages of a named site read besides the one named.
pub const SITE_PAGES: usize = 2;
/// Results of a search whose pages are read.
pub const RESULT_PAGES: usize = 2;
/// Most bytes read of one page.
pub const MAX_PAGE_BYTES: u64 = 3 << 20;
/// Most characters of one page the model gets.
pub const MAX_PAGE_CHARS: usize = 4_000;
/// Most characters of all that was found the model gets: it reads them
/// within its context, with the files (`local_model::MAX_TEXT_CHARS`).
pub const MAX_RESEARCH_CHARS: usize = 14_000;
/// Results of a search the model is told of.
pub const MAX_RESULTS: usize = 6;

/// A page as it was read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    pub title: String,
    /// The page's own summary (`<meta name="description">`).
    pub description: String,
    /// Its text, a line for each paragraph, heading or item.
    pub text: String,
    /// The addresses its links go to, as written.
    pub links: Vec<String>,
}

/// A page the questions were written from, which the panel names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub url: String,
    pub title: String,
}

/// A result of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Where the search for `query` is asked.
pub fn search_address(query: &str) -> String {
    let mut url = Url::parse("https://html.duckduckgo.com/html/").expect("a fixed address");
    url.query_pairs_mut().append_pair("q", query);
    url.into()
}

/// The results on DuckDuckGo's page of results `html`, at most
/// `MAX_RESULTS`: their titles, snippets and the addresses its links send
/// on to (`//duckduckgo.com/l/?uddg=<address>`).
pub fn read_results(html: &str) -> Vec<SearchResult> {
    let mut results: Vec<SearchResult> = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("class=\"result__a\"") {
        let tag_start = rest[..at].rfind('<').unwrap_or(at);
        rest = &rest[tag_start..];
        let Some(close) = tag_end(rest) else { break };
        let href = attribute(&rest[1..close], "href").unwrap_or_default();
        let after = &rest[close + 1..];
        let title = one_line(&decoded(&strip_tags(after.split("</a>").next().unwrap_or_default())));
        // The snippet belongs to this result when it comes before the next one.
        let next = after.find("class=\"result__a\"").unwrap_or(after.len());
        let snippet = after[..next].find("class=\"result__snippet\"").and_then(|s| {
            let snippet = &after[s..next];
            let open = snippet.find('>')? + 1;
            Some(one_line(&decoded(&strip_tags(snippet[open..].split("</a>").next().unwrap_or_default()))))
        });
        rest = after;
        let Some(url) = result_address(&href) else { continue };
        if !title.is_empty() && !results.iter().any(|r| r.url == url) {
            results.push(SearchResult { title, url, snippet: snippet.unwrap_or_default() });
        }
        if results.len() == MAX_RESULTS {
            break;
        }
    }
    results
}

/// The address a result's link `href` sends on to; None for an ad or
/// another link of the search's own.
fn result_address(href: &str) -> Option<String> {
    let absolute = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    let url = Url::parse(&absolute).ok()?;
    let target = if url.host_str().is_some_and(|h| h.ends_with("duckduckgo.com")) {
        if url.path() != "/l/" {
            return None;
        }
        url.query_pairs().find(|(key, _)| key == "uddg").map(|(_, value)| value.into_owned())?
    } else {
        absolute
    };
    let target = Url::parse(&target).ok()?;
    (matches!(target.scheme(), "http" | "https") && !target.host_str().is_some_and(|h| h.ends_with("duckduckgo.com"))).then(|| target.into())
}

/// `html` without its tags.
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

/// The web addresses written in `text` (`https://firma.cz/o-nas`,
/// `www.firma.cz`, `firma.cz`): a request that names a company's site.
pub fn addresses_in(text: &str) -> Vec<Url> {
    let mut found: Vec<Url> = Vec::new();
    for word in text.split_whitespace() {
        let word = word.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | '(' | ')' | '"' | '\'' | '„' | '“' | '<' | '>'));
        let named = word.contains("://") || word.starts_with("www.") || word.rsplit_once('.').is_some_and(|(name, end)| !name.is_empty() && (2..=6).contains(&end.len()) && end.chars().all(|c| c.is_ascii_alphabetic()) && !word.contains('@'));
        if !named {
            continue;
        }
        if let Some(url) = web_address(word).filter(|url| !found.contains(url)) {
            found.push(url);
        }
    }
    found
}

/// The address the quizmaster typed as a web address: `https://` added
/// when it has no scheme (`firma.cz`); None for one that is not `http` or
/// `https` or has no host.
pub fn web_address(typed: &str) -> Option<Url> {
    let typed = typed.trim();
    let typed = if typed.contains("://") { typed.to_string() } else { format!("https://{typed}") };
    let url = Url::parse(&typed).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some_and(|host| host.contains('.') || host == "localhost") && url.username().is_empty() && url.password().is_none())
        .then_some(url)
}

/// Elements whose content is no text of the page.
const SKIPPED: [&str; 9] = ["script", "style", "noscript", "svg", "template", "iframe", "nav", "form", "button"];
/// Elements that begin a new line.
const BLOCKS: [&str; 29] = [
    "p", "div", "br", "li", "ul", "ol", "tr", "td", "th", "table", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article", "header", "footer", "main", "aside",
    "blockquote", "dd", "dt", "figcaption", "address", "hr", "pre",
];

/// The title, summary, text and links of the page `html`.
pub fn read_page(html: &str) -> Page {
    let mut page = Page::default();
    let mut text = String::new();
    let mut rest = html;
    let mut in_title = false;
    while let Some(open) = rest.find('<') {
        let before = &rest[..open];
        if in_title {
            page.title.push_str(before);
        } else {
            text.push_str(before);
        }
        rest = &rest[open..];
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
            continue;
        }
        let Some(close) = tag_end(rest) else { break };
        let tag = &rest[1..close];
        rest = &rest[close + 1..];
        let closing = tag.starts_with('/');
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if !closing && SKIPPED.contains(&name.as_str()) && !tag.ends_with('/') {
            // To its end, also when it holds other tags.
            rest = end_of(rest, &name);
            text.push('\n');
            continue;
        }
        match name.as_str() {
            "title" => in_title = !closing,
            "meta" if attribute(tag, "name").is_some_and(|n| n.eq_ignore_ascii_case("description")) => {
                page.description = attribute(tag, "content").unwrap_or_default();
            }
            "a" if !closing => {
                if let Some(href) = attribute(tag, "href") {
                    page.links.push(href);
                }
            }
            "img" => {
                // A picture's description says what it shows.
                if let Some(alt) = attribute(tag, "alt").filter(|alt| !alt.trim().is_empty()) {
                    text.push_str(&format!(" {alt} "));
                }
            }
            _ => {}
        }
        if BLOCKS.contains(&name.as_str()) {
            text.push('\n');
        } else {
            text.push(' ');
        }
    }
    text.push_str(rest);
    page.title = one_line(&decoded(&page.title));
    page.description = one_line(&decoded(&page.description));
    page.text = lines(&decoded(&text));
    page
}

/// Where the tag that `rest` begins with ends (its `>`, outside quotes).
fn tag_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in rest.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// `rest` after the end tag of `name`, whatever case it is written in.
fn end_of<'a>(rest: &'a str, name: &str) -> &'a str {
    let lower = rest.to_ascii_lowercase();
    let end = format!("</{name}");
    match lower.find(&end) {
        Some(at) => rest[at..].find('>').map_or("", |close| &rest[at + close + 1..]),
        None => "",
    }
}

/// The value of the attribute `name` in the tag `tag`, decoded.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        let starts = at > 0 && lower.as_bytes()[at - 1].is_ascii_whitespace();
        let after = lower[from..].trim_start();
        if !starts || !after.starts_with('=') {
            continue;
        }
        let value = tag[tag.len() - after.len() + 1..].trim_start();
        let value = match value.chars().next() {
            Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or_default(),
            _ => value.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or_default(),
        };
        return Some(decoded(value));
    }
    None
}

/// `text` with its character references (`&amp;`, `&#233;`) as the characters they stand for.
fn decoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[1..].find(';').map(|e| e + 1).filter(|&e| e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let character = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "ndash" => Some('–'),
            "mdash" => Some('—'),
            "hellip" => Some('…'),
            "bdquo" => Some('„'),
            "ldquo" => Some('“'),
            "rdquo" => Some('”'),
            "copy" => Some('©'),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match character {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` as lines with single spaces, without empty ones.
fn lines(text: &str) -> String {
    text.lines().map(one_line).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Words in the addresses of the pages that say most about a company,
/// in Czech and English, best first.
const TELLING: [&str; 24] = [
    "o-nas", "onas", "about", "o-firme", "o-spolecnosti", "spolecnost", "company", "historie", "history", "pribeh", "story", "sluzby", "services", "produkty",
    "products", "nabidka", "tym", "team", "reference", "kariera", "careers", "kontakt", "contact", "aktuality",
];

/// File types a link to which is no page.
const NOT_PAGES: [&str; 16] = ["pdf", "jpg", "jpeg", "png", "gif", "webp", "svg", "zip", "doc", "docx", "xls", "xlsx", "mp4", "mp3", "css", "js"];

/// Of `links` on the page at `base`, at most `max` other pages of the same
/// site worth reading, those whose address says most about the company
/// first (`TELLING`), then the shortest.
pub fn links_to_follow(base: &Url, links: &[String], max: usize) -> Vec<Url> {
    let host = base.host_str().map(|h| h.trim_start_matches("www.").to_ascii_lowercase());
    let mut found: Vec<(usize, Url)> = Vec::new();
    for link in links {
        let Ok(mut url) = base.join(link.trim()) else { continue };
        url.set_fragment(None);
        let same_site = url.host_str().map(|h| h.trim_start_matches("www.").to_ascii_lowercase()) == host;
        let extension = url.path().rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let path = url.path().trim_end_matches('/');
        if !matches!(url.scheme(), "http" | "https") || !same_site || NOT_PAGES.contains(&extension.as_str()) || path == base.path().trim_end_matches('/') || path.is_empty() {
            continue;
        }
        if found.iter().any(|(_, had)| had.path().trim_end_matches('/') == path) {
            continue;
        }
        let lower = url.path().to_ascii_lowercase();
        let rank = TELLING.iter().position(|word| lower.contains(word)).unwrap_or(TELLING.len());
        found.push((rank, url));
    }
    found.sort_by_key(|(rank, url)| (*rank, url.path().len()));
    found.into_iter().map(|(_, url)| url).take(max).collect()
}

/// What was found for the model, at most `max_chars` (`MAX_RESEARCH_CHARS`
/// for a model with room enough): the searches' `results` in at most a
/// quarter of it, then the `pages` read (address and page), each in its
/// share of the rest and at most `MAX_PAGE_CHARS`, under its address and
/// title, without a line an earlier page had (a menu, a footer).
pub fn research_text(results: &[(String, Vec<SearchResult>)], pages: &[(String, Page)], max_chars: usize) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    'searches: for (query, found) in results {
        out.push_str(&format!("Search \"{query}\":\n"));
        for result in found {
            let line = format!("- {} ({}): {}\n", result.title, result.url, result.snippet);
            if out.chars().count() + line.chars().count() > max_chars / 4 {
                break 'searches;
            }
            out.push_str(&line);
        }
        out.push('\n');
    }
    let page_chars = (max_chars.saturating_sub(out.chars().count()) / pages.len().max(1)).min(MAX_PAGE_CHARS);
    for (url, page) in pages {
        let start = out.chars().count();
        out.push_str(&format!("Page {url}"));
        if !page.title.is_empty() {
            out.push_str(&format!(" — {}", page.title));
        }
        out.push('\n');
        if !page.description.is_empty() && seen.insert(page.description.clone()) {
            out.push_str(&page.description);
            out.push('\n');
        }
        for line in page.text.lines() {
            if out.chars().count() - start >= page_chars {
                break;
            }
            if seen.insert(line.to_string()) {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    let text = out.trim_end();
    text.chars().take(max_chars).collect()
}

/// `url` as compared with the pages read before: without its scheme,
/// `www.`, fragment and closing slash, so one page is read once.
pub fn page_key(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else { return url.to_string() };
    parsed.set_fragment(None);
    let host = parsed.host_str().unwrap_or_default().trim_start_matches("www.").to_lowercase();
    let rest = &parsed[url::Position::BeforePath..];
    format!("{host}{}", rest.trim_end_matches('/'))
}

/// The page `bytes` as text, in the character set its `content_type` or
/// its own `<meta charset>` names, UTF-8 when none does.
pub fn page_html(bytes: &[u8], content_type: Option<&str>) -> String {
    let named = |text: &str| -> Option<&'static encoding_rs::Encoding> {
        let lower = text.to_ascii_lowercase();
        let at = lower.find("charset=")? + "charset=".len();
        let name: String = lower[at..].trim_start_matches(['"', '\'']).chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        encoding_rs::Encoding::for_label(name.as_bytes())
    };
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let encoding = content_type.and_then(named).or_else(|| named(&head)).unwrap_or(encoding_rs::UTF_8);
    encoding.decode(bytes).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<!DOCTYPE html><html><head><meta charset="utf-8"><TITLE>Pekárna Novák &amp; syn</TITLE>
        <meta name="description" content="Pečeme od roku 1921 v&nbsp;Brně.">
        <style>p { color: red }</style><script>var x = "<p>not text</p>";</script></head>
        <body><nav><a href="/">Domů</a> <a href="/o-nas/">O nás</a></nav>
        <h1>Naše pekárna</h1><p>Chléb pečeme v peci na dřevo.<br>Denně 2&#160;000 bochníků &#x2013; už 100 let.</p>
        <!-- <p>hidden</p> --><img src="pec.jpg" alt="Pec z roku 1921"><ul><li>Kváskový chléb</li><li>Rohlíky</li></ul>
        <a href='kontakt.html'>Kontakt</a> <a href="https://jinde.cz/x">Jinde</a> <a href="/cenik.pdf">Ceník</a>
        <a href="/produkty#chleb">Produkty</a> <a href="mailto:info@example.com">E-mail</a></body></html>"#;

    #[test]
    fn a_page_gives_its_title_summary_text_and_links() {
        let page = read_page(HTML);
        assert_eq!(page.title, "Pekárna Novák & syn");
        assert_eq!(page.description, "Pečeme od roku 1921 v Brně.");
        assert_eq!(page.text, "Naše pekárna\nChléb pečeme v peci na dřevo.\nDenně 2 000 bochníků – už 100 let.\nPec z roku 1921\nKváskový chléb\nRohlíky\nKontakt Jinde Ceník\nProdukty E-mail");
        // Links in the menu are skipped with it; the others are kept as written.
        assert_eq!(page.links, ["kontakt.html", "https://jinde.cz/x", "/cenik.pdf", "/produkty#chleb", "mailto:info@example.com"]);
    }

    #[test]
    fn the_pages_that_tell_most_about_the_company_are_followed_first() {
        let base = Url::parse("https://www.pekarna.cz/").unwrap();
        let links: Vec<String> = ["kontakt.html", "https://jinde.cz/o-nas", "/cenik.pdf", "/produkty#chleb", "/produkty", "mailto:a@b.cz", "/o-nas/", "https://pekarna.cz/blog/2024/clanek", "/", "#top"]
            .map(String::from)
            .to_vec();
        let followed: Vec<String> = links_to_follow(&base, &links, 3).iter().map(Url::to_string).collect();
        assert_eq!(followed, ["https://www.pekarna.cz/o-nas/", "https://www.pekarna.cz/produkty", "https://www.pekarna.cz/kontakt.html"]);
    }

    #[test]
    fn what_was_found_has_each_line_once_and_limits() {
        let page = |title: &str, text: &str| Page { title: title.into(), description: String::new(), text: text.into(), links: vec![] };
        let pages = vec![("https://a.cz/".to_string(), page("Úvod", "Menu\nPečeme chléb.")), ("https://a.cz/o-nas".to_string(), page("O nás", "Menu\nZaloženo 1921."))];
        let results = vec![("pekárna a".to_string(), vec![SearchResult { title: "Pekárna A".into(), url: "https://a.cz/".into(), snippet: "Chléb od 1921.".into() }])];
        assert_eq!(
            research_text(&results, &pages, MAX_RESEARCH_CHARS),
            "Search \"pekárna a\":\n- Pekárna A (https://a.cz/): Chléb od 1921.\n\nPage https://a.cz/ — Úvod\nMenu\nPečeme chléb.\n\nPage https://a.cz/o-nas — O nás\nZaloženo 1921."
        );
        let lines = "Věta o firmě, která se neopakuje.\n".repeat(1000).lines().enumerate().map(|(i, l)| format!("{i} {l}")).collect::<Vec<_>>().join("\n");
        let long: Vec<(String, Page)> = (0..5).map(|i| (format!("https://a.cz/{i}"), page("", &lines))).collect();
        let text = research_text(&[], &long, MAX_RESEARCH_CHARS);
        assert!(text.chars().count() <= MAX_RESEARCH_CHARS);
        // Each page gets its share, not the first all of it.
        assert!(text.contains("Page https://a.cz/3"));
        // A model with a small context gets less, every page still in it.
        let small = research_text(&results, &long, 3_000);
        assert!(small.chars().count() <= 3_000);
        assert!(small.contains("Page https://a.cz/4") && small.contains("Pekárna A"));
    }

    #[test]
    fn a_page_is_the_same_page_however_its_address_is_written() {
        assert_eq!(page_key("https://www.bakerini.cz/kontakt/"), page_key("http://bakerini.cz/kontakt#mapa"));
        assert_ne!(page_key("https://bakerini.cz/kontakt"), page_key("https://bakerini.cz/produkty"));
        assert_ne!(page_key("https://a.cz/?p=1"), page_key("https://a.cz/?p=2"));
    }

    #[test]
    fn search_results_are_read_with_the_addresses_they_send_on_to() {
        let block = |title: &str, target: &str, snippet: &str| {
            format!(
                r#"<div class="result results_links results_links_deep web-result "><div class="links_main links_deep result__body"><h2 class="result__title">
                <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg={target}&amp;rut=272f">{title}</a></h2>
                <div class="result__extras"><a class="result__url" href="//duckduckgo.com/l/?uddg={target}&amp;rut=272f">www.bakerini.cz</a></div>
                <a class="result__snippet" href="//duckduckgo.com/l/?uddg={target}&amp;rut=272f">{snippet}</a><div class="clear"></div></div></div>"#
            )
        };
        let html = [
            block("Bakerini · Pekárna v Hybešově ulici, Brno", "https%3A%2F%2Fwww.bakerini.cz%2F", "Čerstvé pečivo v srdci Brna, na <b>Hybešově</b> 35."),
            block("CROCUS &amp; syn | Brno", "https%3A%2F%2Fwww.crocus.cz%2F", "Lahodné pekařské výrobky."),
            block("Again", "https%3A%2F%2Fwww.crocus.cz%2F", "Same address."),
            r#"<a class="result__a" href="https://duckduckgo.com/y.js?ad_provider=x">Ad</a>"#.to_string(),
        ]
        .concat();
        let results = read_results(&html);
        assert_eq!(
            results,
            vec![
                SearchResult { title: "Bakerini · Pekárna v Hybešově ulici, Brno".into(), url: "https://www.bakerini.cz/".into(), snippet: "Čerstvé pečivo v srdci Brna, na Hybešově 35.".into() },
                SearchResult { title: "CROCUS & syn | Brno".into(), url: "https://www.crocus.cz/".into(), snippet: "Lahodné pekařské výrobky.".into() },
            ]
        );
        assert_eq!(search_address("pekárna brno"), "https://html.duckduckgo.com/html/?q=pek%C3%A1rna+brno");
    }

    #[test]
    fn addresses_are_found_in_a_request() {
        let found: Vec<String> = addresses_in("Udělej kvíz o firmě podle webu www.pekarna.cz a https://pekarna.cz/o-nas. Piš na info@pekarna.cz, cca 10 otázek.").iter().map(Url::to_string).collect();
        assert_eq!(found, ["https://www.pekarna.cz/", "https://pekarna.cz/o-nas"]);
        assert!(addresses_in("Kvíz o Praze, 10 otázek.").is_empty());
    }

    #[test]
    fn typed_addresses_become_web_addresses() {
        assert_eq!(web_address(" pekarna.cz ").unwrap().as_str(), "https://pekarna.cz/");
        assert_eq!(web_address("http://www.pekarna.cz/o-nas").unwrap().as_str(), "http://www.pekarna.cz/o-nas");
        for bad in ["", "ftp://pekarna.cz", "file:///etc/passwd", "https://user:pw@pekarna.cz", "pekarna", "javascript:alert(1)"] {
            assert!(web_address(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_pages_character_set_is_followed() {
        let cp1250 = [0x50, 0xf8, 0xed, 0x6c, 0x69, 0x9a];
        assert_eq!(page_html(&cp1250, Some("text/html; charset=windows-1250")), "Příliš");
        let declared = [b"<meta charset=\"windows-1250\">".as_slice(), &cp1250].concat();
        assert!(page_html(&declared, Some("text/html")).ends_with("Příliš"));
        assert_eq!(page_html("Příliš".as_bytes(), None), "Příliš");
    }
}
