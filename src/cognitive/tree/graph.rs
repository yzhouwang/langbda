use super::Error;
use super::TreeModel;
use crate::syntax::SyntaxValue;
use graphviz_rust::cmd::{CommandArg, Format};
use graphviz_rust::exec_dot;
use std::fmt::Display;
use std::fmt::Write;

impl<K: Display> SyntaxValue<K> {
    pub fn to_dot_attr(&self) -> String {
        match self {
            SyntaxValue::Item(item) => format!("label=\"{}\", color=lightgreen", item),
            SyntaxValue::Features(features) => {
                let mut entries = Vec::new();
                for (category, value) in features.iter() {
                    match value {
                        Some(value) => entries.push(format!("{}:{}", category, value)),
                        None => entries.push(format!("{}", category)),
                    }
                }
                entries.sort();
                format!("label=\"{}\", color=lightblue", entries.join("\n"))
            }
        }
    }
}

impl<K: Display> TreeModel<K> {
    #[allow(dead_code)]
    pub fn to_dot_graph(&self) -> Result<String, Error> {
        self.to_dot_graph_with_arrows(true)
    }

    pub fn to_dot_graph_with_arrows(&self, show_movement_arrows: bool) -> Result<String, Error> {
        let mut graph = String::new();
        if self.is_empty() {
            return Ok(graph);
        }

        graph.push_str("digraph {{\n");
        graph.push_str("    rankdir=TB;\n");
        graph.push_str("    node [shape=box, style=filled];\n");

        let mut nodes = Vec::new();
        nodes.push(self.get_root());
        while let Some(id) = nodes.pop() {
            let value = self.get_value(id)?.to_dot_attr();
            let value = decorate_with_chain(value, self.get_chain_id(id)?);
            writeln!(&mut graph, r#"    "{}" [{}];"#, id, value)?;

            if let Some(left_id) = self.get_left(id)? {
                writeln!(
                    &mut graph,
                    r#"    "{}" -> "{}" [arrowhead=none];"#,
                    id, left_id
                )?;
                nodes.push(left_id);
            }
            if let Some(right_id) = self.get_right(id)? {
                writeln!(
                    &mut graph,
                    r#"    "{}" -> "{}" [arrowhead=none];"#,
                    id, right_id
                )?;
                nodes.push(right_id);
            }
            if show_movement_arrows {
                if let Some(moved_id) = self.get_moved(id)? {
                    writeln!(
                        &mut graph,
                        r#"    "{}" -> "{}" [style=dashed, constraint=false, color=blue];"#,
                        id, moved_id
                    )?;
                }
            }
        }

        graph.push_str("}}");
        Ok(graph)
    }

    #[allow(dead_code)]
    pub fn to_png(&self, filename: String) -> Result<(), Error> {
        self.to_png_with_arrows(filename, true)
    }

    pub fn to_png_with_arrows(&self, filename: String, show_movement_arrows: bool) -> Result<(), Error> {
        if !is_dot_installed() {
            return Err(Error::GraphvizDotNotInstalled);
        }
        let dot = self.to_dot_graph_with_arrows(show_movement_arrows)?;

        exec_dot(dot, vec![Format::Png.into(), CommandArg::Output(filename)])?;
        Ok(())
    }
}

use std::process::Command;
fn is_dot_installed() -> bool {
    match Command::new("dot").arg("-V").output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

fn decorate_with_chain(attr: String, chain_id: Option<usize>) -> String {
    let Some(chain_id) = chain_id else {
        return attr;
    };

    let marker = format!("\\nCH{}", chain_id);
    if let Some(label_end) = attr.find("\",") {
        format!("{}{}{}", &attr[..label_end], marker, &attr[label_end..])
    } else {
        attr
    }
}
