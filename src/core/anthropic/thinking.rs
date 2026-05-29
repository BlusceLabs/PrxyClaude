#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Text,
    Thinking,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContentChunk {
    pub content_type: ContentType,
    pub content: String,
}

pub struct ThinkTagParser {
    buffer: String,
    in_think_tag: bool,
}

impl ThinkTagParser {
    const OPEN_TAG: &'static str = "<thinking>";
    const CLOSE_TAG: &'static str = "</thinking>";

    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_think_tag: false,
        }
    }

    pub fn in_think_mode(&self) -> bool {
        self.in_think_tag
    }

    pub fn feed(&mut self, content: &str) -> Vec<ContentChunk> {
        self.buffer.push_str(content);
        let mut chunks = Vec::new();

        loop {
            let prev_len = self.buffer.len();
            let chunk = if !self.in_think_tag {
                self.parse_outside_think()
            } else {
                self.parse_inside_think()
            };

            if let Some(c) = chunk {
                chunks.push(c);
            } else if self.buffer.len() == prev_len {
                break;
            }
        }

        chunks
    }

    fn parse_outside_think(&mut self) -> Option<ContentChunk> {
        let think_start = self.buffer.find(Self::OPEN_TAG);
        let orphan_close = self.buffer.find(Self::CLOSE_TAG);

        if let Some(orphan_pos) = orphan_close {
            if think_start.map_or(true, |tp| orphan_pos < tp) {
                let pre_orphan = self.buffer[..orphan_pos].to_string();
                self.buffer = self.buffer[orphan_pos + Self::CLOSE_TAG.len()..].to_string();
                if !pre_orphan.is_empty() {
                    return Some(ContentChunk {
                        content_type: ContentType::Text,
                        content: pre_orphan,
                    });
                }
                return None;
            }
        }

        match think_start {
            None => {
                let last_bracket = self.buffer.rfind('<');
                if let Some(pos) = last_bracket {
                    let potential_tag = &self.buffer[pos..];
                    let tag_len = potential_tag.len();
                    if (tag_len < Self::OPEN_TAG.len()
                        && Self::OPEN_TAG.starts_with(potential_tag))
                        || (tag_len < Self::CLOSE_TAG.len()
                            && Self::CLOSE_TAG.starts_with(potential_tag))
                    {
                        let emit = self.buffer[..pos].to_string();
                        self.buffer = self.buffer[pos..].to_string();
                        if !emit.is_empty() {
                            return Some(ContentChunk {
                                content_type: ContentType::Text,
                                content: emit,
                            });
                        }
                        return None;
                    }
                }

                let emit = self.buffer.clone();
                self.buffer.clear();
                if !emit.is_empty() {
                    Some(ContentChunk {
                        content_type: ContentType::Text,
                        content: emit,
                    })
                } else {
                    None
                }
            }
            Some(pos) => {
                let pre_think = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + Self::OPEN_TAG.len()..].to_string();
                self.in_think_tag = true;
                if !pre_think.is_empty() {
                    Some(ContentChunk {
                        content_type: ContentType::Text,
                        content: pre_think,
                    })
                } else {
                    None
                }
            }
        }
    }

    fn parse_inside_think(&mut self) -> Option<ContentChunk> {
        let think_end = self.buffer.find(Self::CLOSE_TAG);

        match think_end {
            None => {
                let last_bracket = self.buffer.rfind('<');
                if let Some(pos) = last_bracket {
                    if self.buffer.len() - pos < Self::CLOSE_TAG.len() {
                        let potential_tag = &self.buffer[pos..];
                        if Self::CLOSE_TAG.starts_with(potential_tag) {
                            let emit = self.buffer[..pos].to_string();
                            self.buffer = self.buffer[pos..].to_string();
                            if !emit.is_empty() {
                                return Some(ContentChunk {
                                    content_type: ContentType::Thinking,
                                    content: emit,
                                });
                            }
                            return None;
                        }
                    }
                }

                let emit = self.buffer.clone();
                self.buffer.clear();
                if !emit.is_empty() {
                    Some(ContentChunk {
                        content_type: ContentType::Thinking,
                        content: emit,
                    })
                } else {
                    None
                }
            }
            Some(pos) => {
                let thinking_content = self.buffer[..pos].to_string();
                self.buffer = self.buffer[pos + Self::CLOSE_TAG.len()..].to_string();
                self.in_think_tag = false;
                if !thinking_content.is_empty() {
                    Some(ContentChunk {
                        content_type: ContentType::Thinking,
                        content: thinking_content,
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn flush(&mut self) -> Option<ContentChunk> {
        if !self.buffer.is_empty() {
            let content = self.buffer.clone();
            let chunk_type = if self.in_think_tag {
                ContentType::Thinking
            } else {
                ContentType::Text
            };
            self.buffer.clear();
            Some(ContentChunk {
                content_type: chunk_type,
                content,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_text() {
        let mut parser = ThinkTagParser::new();
        let chunks = parser.feed("hello world");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content_type, ContentType::Text);
        assert_eq!(chunks[0].content, "hello world");
    }

    #[test]
    fn test_think_tags() {
        let mut parser = ThinkTagParser::new();
        let chunks = parser.feed("before<thinking>reasoning</thinking>after");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content_type, ContentType::Text);
        assert_eq!(chunks[0].content, "before");
        assert_eq!(chunks[1].content_type, ContentType::Thinking);
        assert_eq!(chunks[1].content, "reasoning");
        assert_eq!(chunks[2].content_type, ContentType::Text);
        assert_eq!(chunks[2].content, "after");
    }

    #[test]
    fn test_partial_tag_across_chunks() {
        let mut parser = ThinkTagParser::new();
        let c1 = parser.feed("hello <thin");
        let c2 = parser.feed("king>reasoning</thin");
        let c3 = parser.feed("king>after");
        assert!(c1.len() >= 1);
        assert!(c2.len() >= 1);
        assert!(c3.len() >= 1);
    }

    #[test]
    fn test_flush() {
        let mut parser = ThinkTagParser::new();
        parser.feed("<thin");
        let flushed = parser.flush();
        assert!(flushed.is_some());
        let chunk = flushed.unwrap();
        assert_eq!(chunk.content, "<thin");
        assert_eq!(chunk.content_type, ContentType::Text);
        assert!(parser.flush().is_none());
    }
}
