/// Accumulates raw bytes across HTTP chunk boundaries and yields only
/// complete lines, buffering any trailing partial line for the next chunk.
///
/// `bytes_stream()` chunk boundaries have no relationship to SSE/NDJSON line
/// boundaries — a `data: {...}` event (or a raw JSON line, for providers
/// like Ollama) can split across two chunks. Parsing each chunk
/// independently (the previous approach) silently drops that event: the
/// truncated JSON in the first chunk fails to parse, and the continuation
/// in the next chunk isn't valid JSON (or a valid "data: " line) on its
/// own either. In practice this most often eats the very first token of a
/// response, since the first network chunk is frequently small.
///
/// Splitting only on complete lines (as found so far) is also UTF-8 safe:
/// `\n` is a single-byte ASCII code point that can never appear as a
/// continuation byte of a multi-byte UTF-8 sequence, so a chunk boundary
/// landing mid-character just delays that line until enough bytes arrive,
/// rather than corrupting it.
#[derive(Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed in the next chunk of bytes and get back every complete line
    /// found so far (across this and previous chunks combined), with any
    /// trailing `\r`/`\n` stripped. An incomplete trailing line is kept
    /// buffered for the next call.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();

        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let mut line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
            line_bytes.pop(); // drop the '\n'
            if line_bytes.last() == Some(&b'\r') {
                line_bytes.pop();
            }
            lines.push(String::from_utf8_lossy(&line_bytes).into_owned());
        }

        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_complete_lines_within_one_chunk() {
        let mut buf = LineBuffer::new();
        let lines = buf.push_chunk(b"data: one\ndata: two\n");
        assert_eq!(lines, vec!["data: one", "data: two"]);
    }

    #[test]
    fn buffers_a_partial_trailing_line_across_chunks() {
        let mut buf = LineBuffer::new();
        // A single SSE event's JSON body split across two chunks.
        let first = buf.push_chunk(b"data: {\"choices\":[{\"delta\":{\"content\":\"Bon");
        assert!(first.is_empty(), "no complete line yet, nothing should be emitted early");

        let second = buf.push_chunk(b"jour\"}}]}\n");
        assert_eq!(second, vec![r#"data: {"choices":[{"delta":{"content":"Bonjour"}}]}"#]);
    }

    #[test]
    fn strips_carriage_return() {
        let mut buf = LineBuffer::new();
        let lines = buf.push_chunk(b"data: hello\r\n");
        assert_eq!(lines, vec!["data: hello"]);
    }

    #[test]
    fn handles_multiple_partial_pushes_before_any_newline() {
        let mut buf = LineBuffer::new();
        assert!(buf.push_chunk(b"data: ").is_empty());
        assert!(buf.push_chunk(b"still going").is_empty());
        let lines = buf.push_chunk(b" done\n");
        assert_eq!(lines, vec!["data: still going done"]);
    }

    #[test]
    fn does_not_corrupt_a_multibyte_utf8_char_split_across_chunks() {
        let mut buf = LineBuffer::new();
        let full = "data: caf\u{e9} done\n".as_bytes().to_vec(); // "café"
        // Split right in the middle of the 2-byte 'é' sequence.
        let split_at = full.iter().position(|&b| b == b'f').unwrap() + 2; // mid 'é'
        let first = buf.push_chunk(&full[..split_at]);
        assert!(first.is_empty());
        let second = buf.push_chunk(&full[split_at..]);
        assert_eq!(second, vec!["data: caf\u{e9} done"]);
    }
}
