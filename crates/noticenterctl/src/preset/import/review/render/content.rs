//! Review headings, command entries, file metadata, and escaped bodies

use std::fmt;

use super::super::checks::ImportedExecContent;
use super::model::{
    ReviewDetail, MAX_COMPLETE_REVIEW_COMMAND_BYTES, MAX_COMPLETE_REVIEW_OUTPUT_BYTES,
    MAX_COMPLETE_REVIEW_TEXT_BYTES,
};
use super::style::ReviewStyle;
use super::style::{write_styled, ReviewTone};

pub(super) fn write_exec_review(
    output: &mut impl fmt::Write,
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
    detail: ReviewDetail,
    complete: bool,
) -> fmt::Result {
    write_styled(output, style, ReviewTone::Title, |output| {
        output.write_str("UnixNotis preset executable content review")
    })?;
    output.write_str("\n\n")?;
    write_styled(output, style, ReviewTone::Warning, |output| {
        output.write_str("This preset contains executable commands or bundled scripts")
    })?;
    output.write_char('\n')?;
    write_styled(output, style, ReviewTone::Note, |output| {
        output.write_str("Only continue if the source is trusted")
    })?;
    output.write_str("\n\n")?;

    write_review_status(output, style, complete)?;
    output.write_char('\n')?;

    match detail {
        ReviewDetail::Full => write_review_entries(output, exec_content, style, false)?,
        ReviewDetail::Metadata => {
            write_styled(output, style, ReviewTone::Note, |output| {
                write!(
                    output,
                    "Review bodies are omitted because complete output exceeds {MAX_COMPLETE_REVIEW_OUTPUT_BYTES} bytes"
                )
            })?;
            output.write_char('\n')?;
            write_review_entries(output, exec_content, style, true)?;
        }
        ReviewDetail::Summary => {
            // Counts remain useful if unusually large paths make the metadata inventory too large
            write_styled(output, style, ReviewTone::Note, |output| {
                write!(
                    output,
                    "Review metadata is omitted because it exceeds {MAX_COMPLETE_REVIEW_OUTPUT_BYTES} bytes; {} commands; {} bundled files",
                    exec_content.commands.len(),
                    exec_content.files.len()
                )
            })?;
            output.write_char('\n')?;
        }
    }

    Ok(())
}

fn write_review_status(
    output: &mut impl fmt::Write,
    style: ReviewStyle,
    complete: bool,
) -> fmt::Result {
    if complete {
        write_styled(output, style, ReviewTone::Note, |output| {
            output.write_str("Review status: complete")
        })
    } else {
        write_styled(output, style, ReviewTone::Warning, |output| {
            output.write_str("Review status: incomplete; ordinary approval is disabled")
        })
    }
}

fn write_review_entries(
    output: &mut impl fmt::Write,
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
    metadata_only: bool,
) -> fmt::Result {
    write_command_entries(output, exec_content, style, metadata_only)?;
    write_file_entries(output, exec_content, style, metadata_only)
}

fn write_command_entries(
    output: &mut impl fmt::Write,
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
    metadata_only: bool,
) -> fmt::Result {
    if exec_content.commands.is_empty() {
        return Ok(());
    }

    output.write_char('\n')?;
    write_styled(output, style, ReviewTone::Section, |output| {
        output.write_str("Command entries")
    })?;
    output.write_char('\n')?;
    for command in &exec_content.commands {
        output.write_str("  ")?;
        write_styled(output, style, ReviewTone::Slot, |output| {
            write_escaped_review_text(output, &command.slot, false)
        })?;
        output.write_str(" = ")?;
        if !metadata_only && command.command.len() <= MAX_COMPLETE_REVIEW_COMMAND_BYTES {
            // Full command text is required because meaningful work may be hidden at the end
            write_styled(output, style, ReviewTone::Command, |output| {
                write_escaped_review_text(output, &command.command, false)
            })?;
        } else {
            write_command_metadata(output, &command.command, style, metadata_only)?;
        }
        output.write_char('\n')?;
    }
    Ok(())
}

fn write_command_metadata(
    output: &mut impl fmt::Write,
    command: &str,
    style: ReviewStyle,
    aggregate_limited: bool,
) -> fmt::Result {
    let digest = blake3::hash(command.as_bytes());
    write_styled(output, style, ReviewTone::Note, |output| {
        if aggregate_limited {
            write!(
                output,
                "<command body omitted; {} bytes; BLAKE3 {}>",
                command.len(),
                digest.to_hex()
            )
        } else {
            write!(
                output,
                "<command not displayed; {} bytes; BLAKE3 {}>",
                command.len(),
                digest.to_hex()
            )
        }
    })
}

fn write_file_entries(
    output: &mut impl fmt::Write,
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
    metadata_only: bool,
) -> fmt::Result {
    if exec_content.files.is_empty() {
        return Ok(());
    }

    output.write_char('\n')?;
    write_styled(output, style, ReviewTone::Section, |output| {
        output.write_str("Bundled files available to commands")
    })?;
    output.write_char('\n')?;
    for file in &exec_content.files {
        let digest = blake3::hash(&file.contents);
        output.write_char('\n')?;
        write_styled(output, style, ReviewTone::FileHeader, |output| {
            output.write_str("== ")?;
            write_escaped_review_text(
                output,
                file.relative_path.to_string_lossy().as_ref(),
                false,
            )?;
            write!(
                output,
                " (mode {:o}, {} bytes, BLAKE3 {}) ==",
                file.mode,
                file.contents.len(),
                digest.to_hex()
            )
        })?;
        output.write_char('\n')?;

        if metadata_only {
            write_styled(output, style, ReviewTone::Note, |output| {
                output.write_str(
                    "<file body omitted because the aggregate review limit was exceeded>",
                )
            })?;
        } else {
            write_file_body(output, file.contents.as_slice(), style)?;
        }
        output.write_char('\n')?;
    }
    Ok(())
}

fn write_file_body(
    output: &mut impl fmt::Write,
    contents: &[u8],
    style: ReviewStyle,
) -> fmt::Result {
    match std::str::from_utf8(contents) {
        // Small text files are represented completely with unsafe controls escaped
        Ok(text) if contents.len() <= MAX_COMPLETE_REVIEW_TEXT_BYTES => {
            write_styled(output, style, ReviewTone::FileBody, |output| {
                write_escaped_review_text(output, text, true)
            })
        }
        Ok(_) => write_styled(output, style, ReviewTone::Note, |output| {
            write!(
                output,
                "<text not displayed because it exceeds {MAX_COMPLETE_REVIEW_TEXT_BYTES} bytes>"
            )
        }),
        // Binary content is represented by its exact size and digest in the header
        Err(_) => write_styled(output, style, ReviewTone::Note, |output| {
            output.write_str("<binary content represented by metadata above>")
        }),
    }
}

fn write_escaped_review_text(
    output: &mut impl fmt::Write,
    value: &str,
    keep_newlines: bool,
) -> fmt::Result {
    for ch in value.chars() {
        match ch {
            '\n' if keep_newlines => output.write_char('\n')?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            '\\' => output.write_str("\\\\")?,
            _ if ch.is_control() || is_bidi_control(ch) => {
                // Visible escapes preserve every character without allowing terminal effects
                write!(output, "\\u{{{:04x}}}", u32::from(ch))?;
            }
            _ => output.write_char(ch)?,
        }
    }
    Ok(())
}

const fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}
