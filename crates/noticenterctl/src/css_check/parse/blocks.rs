use super::types::CssBlock;

pub(in super::super) fn strip_css_comments(input: &str) -> String {
    // Comment bytes get replaced instead of removed so offsets still line up later
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            // Stay in comment mode until the closing marker is found
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                output.push(' ');
                output.push(' ');
                chars.next();
                in_comment = false;
                continue;
            }
            if ch == '\n' || ch == '\r' {
                // Newlines need to stay in place so line numbers still match
                output.push(ch);
            } else {
                output.push(' ');
            }
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            // Replace the opening marker too so offsets stay aligned with the source
            output.push(' ');
            output.push(' ');
            chars.next();
            in_comment = true;
            continue;
        }
        output.push(ch);
    }
    output
}

pub(in super::super) fn next_css_block(
    bytes: &[u8],
    start: usize,
) -> Option<(String, String, usize)> {
    // Older callers only need the text, so this stays as a small adapter
    next_css_block_with_offsets(bytes, start).map(|block| (block.selector, block.block, block.next))
}

pub(in super::super) fn next_css_block_with_offsets(
    bytes: &[u8],
    start: usize,
) -> Option<CssBlock> {
    // This scanner only needs to find balanced blocks
    let mut selector_start = start;
    while selector_start < bytes.len() && bytes[selector_start].is_ascii_whitespace() {
        selector_start += 1;
    }

    let mut index = selector_start;
    let mut in_string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            if byte == quote {
                in_string = None;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            in_string = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'{' {
            // Selector text is copied once so later passes can normalize it freely
            let selector = String::from_utf8_lossy(&bytes[selector_start..index]).to_string();
            let mut depth = 1usize;
            index += 1;
            let block_start = index;
            while index < bytes.len() {
                let byte = bytes[index];
                if let Some(quote) = in_string {
                    if byte == quote {
                        in_string = None;
                    }
                    index += 1;
                    continue;
                }
                if byte == b'"' || byte == b'\'' {
                    in_string = Some(byte);
                    index += 1;
                    continue;
                }
                if byte == b'{' {
                    depth += 1;
                } else if byte == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        let block = String::from_utf8_lossy(&bytes[block_start..index]).to_string();
                        return Some(CssBlock {
                            selector,
                            block,
                            next: index + 1,
                            selector_start,
                            block_start,
                        });
                    }
                }
                index += 1;
            }
            break;
        }
        index += 1;
    }
    None
}
