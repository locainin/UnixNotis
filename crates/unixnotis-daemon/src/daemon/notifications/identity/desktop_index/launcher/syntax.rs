//! Fail-closed Bash syntax analysis for literal final `exec` targets

use std::path::{Component, Path, PathBuf};

use tree_sitter::{Node, Parser};

const MAX_SYNTAX_NODES: usize = 16_384;
const MAX_EXEC_ARGUMENTS: usize = 128;
const FORBIDDEN_COMMANDS: [&str; 7] = [
    ".", "alias", "builtin", "command", "enable", "eval", "source",
];

pub(super) fn literal_final_exec_target(source: &[u8]) -> Option<PathBuf> {
    validate_shell_shebang(source)?;
    let source_text = std::str::from_utf8(source).ok()?;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source_text, None)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    if root.kind() != "program" {
        return None;
    }

    // Iterative traversal applies one resource bound to nested substitutions and blocks
    let nodes = syntax_nodes(root)?;
    if nodes
        .iter()
        .any(|node| matches!(node.kind(), "function_definition" | "heredoc_redirect"))
    {
        return None;
    }

    let commands = nodes
        .iter()
        .filter(|node| node.kind() == "command")
        .copied()
        .collect::<Vec<_>>();
    for command in &commands {
        let name = command_name(*command, source)?;
        if FORBIDDEN_COMMANDS.contains(&name) {
            return None;
        }
    }

    let mut exec_commands = commands
        .into_iter()
        .filter(|command| command_name(*command, source) == Some("exec"));
    let exec = exec_commands.next()?;
    if exec_commands.next().is_some() {
        return None;
    }
    if exec.parent().map(|node| node.kind()) != Some("program") {
        return None;
    }
    if last_top_level_statement(root)? != exec || exec.child_by_field_name("redirect").is_some() {
        return None;
    }

    let mut cursor = exec.walk();
    let arguments = exec
        .children_by_field_name("argument", &mut cursor)
        .collect::<Vec<_>>();
    if arguments.is_empty() {
        return None;
    }
    if arguments.len() > MAX_EXEC_ARGUMENTS {
        return None;
    }
    literal_absolute_path(arguments[0], source)
}

fn validate_shell_shebang(source: &[u8]) -> Option<()> {
    let first_line = source.split(|byte| *byte == b'\n').next()?;
    let first_line = std::str::from_utf8(first_line).ok()?.trim_end_matches('\r');
    let command = first_line.strip_prefix("#!")?.trim();
    let words = command.split_ascii_whitespace().collect::<Vec<_>>();
    match words.as_slice() {
        ["/bin/sh" | "/usr/bin/sh" | "/bin/bash" | "/usr/bin/bash"]
        | ["/usr/bin/env", "sh" | "bash"] => Some(()),
        _ => None,
    }
}

fn syntax_nodes(root: Node<'_>) -> Option<Vec<Node<'_>>> {
    let mut pending = vec![root];
    let mut nodes = Vec::new();
    while let Some(node) = pending.pop() {
        if nodes.len() >= MAX_SYNTAX_NODES {
            return None;
        }
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
        nodes.push(node);
    }
    Some(nodes)
}

fn command_name<'source>(command: Node<'_>, source: &'source [u8]) -> Option<&'source str> {
    command.child_by_field_name("name")?.utf8_text(source).ok()
}

fn last_top_level_statement(root: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|node| node.kind() != "comment")
        .last()
}

fn literal_absolute_path(node: Node<'_>, source: &[u8]) -> Option<PathBuf> {
    // Only an unquoted word without expansions can select the authenticated target
    if node.kind() != "word" {
        return None;
    }
    if node.named_child_count() != 0 {
        return None;
    }
    let value = node.utf8_text(source).ok()?;
    if value.contains(['*', '?', '[', ']', '{', '}', '~', '$', '`']) {
        return None;
    }
    let path = Path::new(value);
    let mut components = path.components();
    if components.next() != Some(Component::RootDir)
        || !components.all(|component| matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(path.to_path_buf())
}
