//! Incremental parsing of streamed HTTP response bodies.

use crate::state::{ChatMessage, ChatRole};

/// Callback that receives each piece of a reply as it is generated
pub type OnDelta<'a> = &'a mut (dyn FnMut(&str) + Send);

/// Splits a byte stream into lines. Bytes are buffered until a full line
/// arrives, so multi-byte characters split across chunks decode correctly.
#[derive(Default)]
pub(crate) struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    /// Add a chunk and return the complete lines it finished (without `\n` or `\r\n`)
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = &line[..line.len() - 1];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            lines.push(String::from_utf8_lossy(line).into_owned());
        }
        lines
    }

    /// Return any final line that wasn't terminated by a newline
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            None
        } else {
            let line = String::from_utf8_lossy(&self.buf).into_owned();
            self.buf.clear();
            Some(line)
        }
    }
}

/// Parses Server-Sent Events, returning the `data` payload of each event
#[derive(Default)]
pub(crate) struct SseParser {
    lines: LineBuffer,
    data: Vec<String>,
}

impl SseParser {
    /// Add a chunk and return the data of every event it completed
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut events = Vec::new();
        for line in self.lines.push(chunk) {
            self.handle_line(&line, &mut events);
        }
        events
    }

    /// Return the data of a final event not followed by a blank line
    pub fn finish(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if let Some(line) = self.lines.finish() {
            self.handle_line(&line, &mut events);
        }
        if !self.data.is_empty() {
            events.push(self.data.join("\n"));
            self.data.clear();
        }
        events
    }

    fn handle_line(&mut self, line: &str, events: &mut Vec<String>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                events.push(self.data.join("\n"));
                self.data.clear();
            }
        } else if let Some(value) = line.strip_prefix("data:") {
            self.data
                .push(value.strip_prefix(' ').unwrap_or(value).to_string());
        }
        // `event:`, `id:`, `retry:` and `:` comment lines are not needed
    }
}

/// The wire name of a chat role
pub(crate) fn role_name(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

/// `{"role", "content"}` objects for OpenAI-style APIs, with the system prompt first
pub(crate) fn messages_with_system(
    system: &str,
    messages: &[ChatMessage],
) -> Vec<serde_json::Value> {
    std::iter::once(serde_json::json!({"role": "system", "content": system}))
        .chain(
            messages
                .iter()
                .map(|m| serde_json::json!({"role": role_name(m.role), "content": m.content})),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_buffer_handles_split_lines_and_crlf() {
        let mut lines = LineBuffer::default();
        assert!(lines.push(b"hel").is_empty());
        assert_eq!(lines.push(b"lo\r\nwor"), ["hello"]);
        assert_eq!(lines.push(b"ld\n\n"), ["world", ""]);
        assert_eq!(lines.finish(), None);
        lines.push(b"tail");
        assert_eq!(lines.finish(), Some("tail".to_string()));
    }

    #[test]
    fn test_line_buffer_keeps_utf8_split_across_chunks() {
        let text = "fe y esperanza ✎\n".as_bytes();
        let (a, b) = text.split_at(text.len() - 3); // splits the 3-byte ✎
        let mut lines = LineBuffer::default();
        assert!(lines.push(a).is_empty());
        assert_eq!(lines.push(b), ["fe y esperanza ✎"]);
    }

    #[test]
    fn test_sse_parser_events() {
        let mut sse = SseParser::default();
        let events = sse.push(
            b"event: message_start\ndata: {\"a\":1}\n\n: comment\ndata: line1\ndata: line2\n\ndata: [DO",
        );
        assert_eq!(events, ["{\"a\":1}", "line1\nline2"]);
        assert_eq!(sse.push(b"NE]\n\n"), ["[DONE]"]);
        sse.push(b"data: last");
        assert_eq!(sse.finish(), ["last"]);
    }

    #[test]
    fn test_messages_with_system() {
        let messages = [ChatMessage {
            role: ChatRole::User,
            content: "Hi".to_string(),
        }];
        let json = messages_with_system("Be brief", &messages);
        assert_eq!(
            serde_json::Value::Array(json),
            serde_json::json!([
                {"role": "system", "content": "Be brief"},
                {"role": "user", "content": "Hi"}
            ])
        );
    }
}
