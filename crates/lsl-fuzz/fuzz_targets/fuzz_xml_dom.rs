//! Fuzz XML DOM operations with arbitrary sequences of mutations.

#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use lsl_core::xml_dom::XmlNode;

#[derive(Arbitrary, Debug)]
enum XmlOp {
    AppendChild(String),
    AppendChildValue(String, String),
    PrependChild(String),
    SetValue(String),
    SetName(String),
    SetChildValue(String, String),
    GetChild(String),
    GetChildValue(String),
    RemoveChildNamed(String),
    FirstChild,
    LastChild,
    DeepClone,
    ToXml,
}

fuzz_target!(|ops: Vec<XmlOp>| {
    let root = XmlNode::new("root");
    let mut current = root.clone();

    for op in ops {
        match op {
            XmlOp::AppendChild(name) => {
                if name.len() < 64 && !name.is_empty() {
                    current = root.append_child(&name);
                }
            }
            XmlOp::AppendChildValue(name, value) => {
                if name.len() < 64 && value.len() < 256 && !name.is_empty() {
                    root.append_child_value(&name, &value);
                }
            }
            XmlOp::PrependChild(name) => {
                if name.len() < 64 && !name.is_empty() {
                    root.prepend_child(&name);
                }
            }
            XmlOp::SetValue(val) => {
                if val.len() < 256 {
                    current.set_value(&val);
                }
            }
            XmlOp::SetName(name) => {
                if name.len() < 64 && !name.is_empty() {
                    current.set_name(&name);
                }
            }
            XmlOp::SetChildValue(name, val) => {
                if name.len() < 64 && val.len() < 256 && !name.is_empty() {
                    root.set_child_value(&name, &val);
                }
            }
            XmlOp::GetChild(name) => {
                let _ = root.child(&name);
            }
            XmlOp::GetChildValue(name) => {
                let _ = root.child_value(&name);
            }
            XmlOp::RemoveChildNamed(name) => {
                root.remove_child_named(&name);
            }
            XmlOp::FirstChild => {
                let ch = root.first_child();
                if !ch.is_empty() {
                    current = ch;
                }
            }
            XmlOp::LastChild => {
                let ch = root.last_child();
                if !ch.is_empty() {
                    current = ch;
                }
            }
            XmlOp::DeepClone => {
                let _ = root.deep_clone();
            }
            XmlOp::ToXml => {
                let _ = root.to_xml();
            }
        }
    }

    // Always try to serialize at the end
    let _ = root.to_xml();
});
