use crate::util;
use crate::value::PrimitiveValue;
use crate::value::VarName;
use anyhow::*;
use element::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use xml_ext::*;

pub mod element;
pub mod xml_ext;

#[macro_export]
macro_rules! ensure_xml_tag_is {
    ($element:ident, $name:literal) => {
        ensure!(
            $element.tag_name() == $name,
            anyhow!(
                "{} | Tag needed to be of type '{}', but was: {}",
                $element.text_pos(),
                $name,
                $element.as_tag_string()
            )
        )
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptVar {
    pub name: VarName,
    pub command: String,
    pub interval: std::time::Duration,
}

impl ScriptVar {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "script-var");

        let name = VarName(xml.attr("name")?.to_owned());
        let interval = util::parse_duration(xml.attr("interval")?)?;
        let command = xml.only_child()?.as_text()?.text();
        Ok(ScriptVar { name, interval, command })
    }
}

#[derive(Debug, Clone)]
pub struct EwwConfig {
    widgets: HashMap<String, WidgetDefinition>,
    windows: HashMap<WindowName, EwwWindowDefinition>,
    initial_variables: HashMap<VarName, PrimitiveValue>,
    script_vars: Vec<ScriptVar>,
}

impl EwwConfig {
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let document = roxmltree::Document::parse(&content)?;

        let result = EwwConfig::from_xml_element(XmlNode::from(document.root_element()).as_element()?.clone());
        result
    }

    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        let definitions = xml
            .child("definitions")?
            .child_elements()
            .map(|child| {
                let def = WidgetDefinition::from_xml_element(child)?;
                Ok((def.name.clone(), def))
            })
            .collect::<Result<HashMap<_, _>>>()
            .context("error parsing widget definitions")?;

        let windows = xml
            .child("windows")?
            .child_elements()
            .map(|child| {
                Ok((
                    WindowName(child.attr("name")?.to_owned()),
                    EwwWindowDefinition::from_xml_element(child)?,
                ))
            })
            .collect::<Result<HashMap<_, _>>>()
            .context("error parsing window definitions")?;

        let variables_block = xml.child("variables").ok();

        let mut initial_variables = HashMap::new();
        let mut script_vars = Vec::new();
        if let Some(variables_block) = variables_block {
            for node in variables_block.child_elements() {
                match node.tag_name() {
                    "var" => {
                        initial_variables.insert(
                            VarName(node.attr("name")?.to_owned()),
                            PrimitiveValue::parse_string(
                                &node
                                    .only_child()
                                    .map(|c| c.as_text_or_sourcecode())
                                    .unwrap_or_else(|_| String::new()),
                            ),
                        );
                    }
                    "script-var" => {
                        script_vars.push(ScriptVar::from_xml_element(node)?);
                    }
                    _ => bail!("Illegal element in variables block: {}", node.as_tag_string()),
                }
            }
        }

        Ok(EwwConfig {
            widgets: definitions,
            windows,
            initial_variables,
            script_vars,
        })
    }

    // TODO this is kinda ugly
    pub fn generate_initial_state(&self) -> Result<HashMap<VarName, PrimitiveValue>> {
        let mut vars = self
            .script_vars
            .iter()
            .map(|var| Ok((var.name.clone(), crate::eww_state::run_command(&var.command)?)))
            .collect::<Result<HashMap<_, _>>>()?;
        vars.extend(self.get_default_vars().clone());
        Ok(vars)
    }

    pub fn get_widgets(&self) -> &HashMap<String, WidgetDefinition> {
        &self.widgets
    }
    pub fn get_windows(&self) -> &HashMap<WindowName, EwwWindowDefinition> {
        &self.windows
    }
    pub fn get_default_vars(&self) -> &HashMap<VarName, PrimitiveValue> {
        &self.initial_variables
    }
    pub fn get_script_vars(&self) -> &Vec<ScriptVar> {
        &self.script_vars
    }
}

#[repr(transparent)]
#[derive(
    Debug, Clone, Hash, PartialEq, Eq, derive_more::AsRef, derive_more::From, derive_more::FromStr, Serialize, Deserialize,
)]
pub struct WindowName(String);

impl std::borrow::Borrow<str> for WindowName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WindowName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EwwWindowDefinition {
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub widget: WidgetUse,
}

impl EwwWindowDefinition {
    pub fn from_xml_element(xml: XmlElement) -> Result<Self> {
        ensure_xml_tag_is!(xml, "window");

        let size_node = xml.child("size")?;
        let size = (size_node.attr("x")?.parse()?, size_node.attr("y")?.parse()?);
        let pos_node = xml.child("pos")?;
        let position = (pos_node.attr("x")?.parse()?, pos_node.attr("y")?.parse()?);

        let widget = WidgetUse::from_xml_node(xml.child("widget")?.only_child()?)?;
        Ok(EwwWindowDefinition { position, size, widget })
    }
}
