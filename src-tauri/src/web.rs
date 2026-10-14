//! Bounded, read-only projection of supplied HTML. This parser never requests links.
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use std::cell::{Cell, RefCell};

pub struct WebProjection {
    pub title: String,
    pub markdown: String,
    pub semantic_text: String,
    pub line_count: usize,
    pub detail: String,
}

#[derive(Default)]
struct HtmlSink {
    base_url: String,
    body: RefCell<String>,
    title: RefCell<String>,
    in_title: Cell<bool>,
    hidden: RefCell<Vec<String>>,
    anchor: RefCell<Option<(String, String)>>,
}

impl TokenSink for HtmlSink {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        match token {
            Token::CharacterTokens(chars) => {
                if self.in_title.get() {
                    self.title.borrow_mut().push_str(&chars);
                } else if self.hidden.borrow().is_empty() {
                    if let Some((_, text)) = self.anchor.borrow_mut().as_mut() {
                        text.push_str(&chars);
                    } else {
                        self.body.borrow_mut().push_str(&chars);
                    }
                }
            }
            Token::TagToken(tag) => {
                let name = tag.name.to_string();
                match tag.kind {
                    TagKind::StartTag => {
                        if matches!(
                            name.as_str(),
                            "script" | "style" | "noscript" | "template" | "svg"
                        ) {
                            self.hidden.borrow_mut().push(name.clone());
                            return TokenSinkResult::Continue;
                        }
                        if !self.hidden.borrow().is_empty() {
                            return TokenSinkResult::Continue;
                        }
                        if name == "title" {
                            self.in_title.set(true);
                        }
                        if name == "a" && self.anchor.borrow().is_none() {
                            let href = tag
                                .attrs
                                .iter()
                                .find(|attribute| attribute.name.local.as_ref() == "href")
                                .map(|attribute| attribute.value.to_string())
                                .unwrap_or_default();
                            *self.anchor.borrow_mut() = Some((href, String::new()));
                        }
                        if is_block(&name) {
                            append_break(&mut self.body.borrow_mut());
                        }
                    }
                    TagKind::EndTag => {
                        let hidden_index = {
                            self.hidden
                                .borrow()
                                .iter()
                                .rposition(|hidden| hidden == &name)
                        };
                        if let Some(index) = hidden_index {
                            self.hidden.borrow_mut().truncate(index);
                            return TokenSinkResult::Continue;
                        }
                        if !self.hidden.borrow().is_empty() {
                            return TokenSinkResult::Continue;
                        }
                        if name == "title" {
                            self.in_title.set(false);
                        }
                        if name == "a" {
                            if let Some((href, text)) = self.anchor.borrow_mut().take() {
                                let text = collapse_space(&text);
                                let href = collapse_space(&href);
                                if !text.is_empty() {
                                    self.body.borrow_mut().push_str(&text);
                                }
                                if let Some(destination) = resolve_href(&href, &self.base_url) {
                                    self.body.borrow_mut().push_str(" (");
                                    self.body.borrow_mut().push_str(&destination);
                                    self.body.borrow_mut().push(')');
                                } else if !href.is_empty() {
                                    self.body.borrow_mut().push_str(" (");
                                    self.body.borrow_mut().push_str(&href);
                                    self.body.borrow_mut().push(')');
                                }
                            }
                        }
                        if is_block(&name) {
                            append_break(&mut self.body.borrow_mut());
                        }
                    }
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

pub fn project(bytes: &[u8], final_url: &str) -> Result<WebProjection, String> {
    let html = std::str::from_utf8(bytes).map_err(|_| {
        "The HTML response is not valid UTF-8; the downloaded original is retained.".to_owned()
    })?;
    let sink = HtmlSink {
        base_url: final_url.to_owned(),
        ..HtmlSink::default()
    };
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    let tokenizer = Tokenizer::new(sink, TokenizerOpts::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let title = collapse_space(&tokenizer.sink.title.borrow());
    let raw_body = tokenizer.sink.body.borrow();
    let body = normalize_blocks(&raw_body);
    let line_count = body.lines().count();
    let markdown = if body.is_empty() {
        format!("# {title}\n\n[Open original](ORIGINAL_ASSET)\n\nThe supplied HTML contained no readable text.\n")
    } else {
        format!("# {title}\n\n[Open original](ORIGINAL_ASSET)\n\n## Retrieved page text\n\n```text\n{body}\n```\n")
    };
    Ok(WebProjection {
        title,
        markdown,
        semantic_text: body,
        line_count,
        detail: if line_count == 0 {
            "The downloaded HTML is retained; no readable text was found.".into()
        } else {
            "Visible HTML text and link occurrences were projected; the exact downloaded HTML remains available as the original.".into()
        },
    })
}

fn normalize_blocks(input: &str) -> String {
    let mut paragraphs = Vec::new();
    for block in input.split("\n\n") {
        let text = collapse_space(block);
        if !text.is_empty() {
            paragraphs.push(text);
        }
    }
    paragraphs.join("\n\n")
}

fn resolve_href(href: &str, final_url: &str) -> Option<String> {
    let base = reqwest::Url::parse(final_url).ok()?;
    let url = base.join(href).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

fn collapse_space(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_break(output: &mut String) {
    if !output.ends_with("\n\n") {
        output.push_str("\n\n");
    }
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "blockquote"
            | "br"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "fieldset"
            | "figcaption"
            | "figure"
            | "footer"
            | "form"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}
