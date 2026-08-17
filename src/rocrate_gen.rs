/*
* This file is align with the `rocrate.py` of vre_rocrate library.
*/
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, todo};

use crate::{FileEntry, LaunchInput, SlotTyp, SlotValue, ToolMeta};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VreLaunchRequest {
    pub tool: ToolMeta,
    pub input: LaunchInput,

    #[serde(default)]
    pub runtime_platform: Option<String>,
}

// VRE types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VreType {
    Galaxy,
    Oscar,
    Scipion,
    Binder,
    Jupyter,
    Vip,
    Mddash,
    Sciencemesh,
    Rrp,
}

impl VreType {
    pub fn programming_language(self) -> &'static str {
        match self {
            Self::Galaxy => "https://galaxyproject.org/",
            Self::Oscar => "https://oscar.grycap.net/",
            Self::Scipion => "http://scipion.i2pc.es/",
            Self::Binder => "https://jupyter.org/binder/",
            Self::Jupyter => "https://jupyter.org",
            Self::Vip => "https://vip.creatis.insa-lyon.fr/",
            Self::Mddash => "https://github.com/CERIT-SC/mddash",
            Self::Sciencemesh => "https://eosc.cernbox.cern.ch",
            Self::Rrp => "",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Galaxy => "Galaxy",
            Self::Oscar => "OSCAR",
            Self::Scipion => "Scipion",
            Self::Binder => "Binder",
            Self::Jupyter => "Jupyter Notebook",
            Self::Vip => "VIP",
            Self::Mddash => "MDDash",
            Self::Sciencemesh => "Jupyter Notebook",
            Self::Rrp => "",
        }
    }

    pub fn language_url(self) -> &'static str {
        match self {
            Self::Galaxy => "https://galaxyproject.org/",
            Self::Oscar => "https://oscar.grycap.net/",
            Self::Scipion => "http://scipion.i2pc.es/",
            Self::Binder => "https://jupyter.org/binder/",
            Self::Jupyter => "https://jupyter.org",
            Self::Vip => "https://vip.creatis.insa-lyon.fr/",
            Self::Mddash => "https://github.com/CERIT-SC/mddash",
            Self::Sciencemesh => "https://jupyter.org/",
            Self::Rrp => "",
        }
    }

    pub fn default_runtime_platform(self) -> &'static str {
        match self {
            Self::Galaxy => "https://usegalaxy.eu/",
            Self::Binder => "https://mybinder.org/",
            Self::Jupyter => "https://jupyterhub.egi.eu/",
            Self::Oscar => "https://oscar.grycap.net/",
            Self::Vip => "https://vip.creatis.insa-lyon.fr/",
            Self::Scipion => "http://scipion.i2pc.es/",
            Self::Mddash => "https://mddash.cerit-sc.cz/",
            Self::Sciencemesh => "https://eosc.cernbox.cern.ch",
            Self::Rrp => "https://rrp-eosc.ethz.ch/",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "galaxy" => Some(Self::Galaxy),
            "oscar" => Some(Self::Oscar),
            "scipion" => Some(Self::Scipion),
            "binder" => Some(Self::Binder),
            "jupyter" => Some(Self::Jupyter),
            "vip" => Some(Self::Vip),
            "mddash" => Some(Self::Mddash),
            "sciencemesh" => Some(Self::Sciencemesh),
            "rrp" => Some(Self::Rrp),
            _ => None,
        }
    }
}

// VRE resolution
fn tool_type_to_vre_type(value: &str) -> Option<VreType> {
    match value {
        "egi-replay" => Some(VreType::Binder),
        "binder" => Some(VreType::Binder),
        "galaxy" => Some(VreType::Galaxy),
        "galaxy_workflow" => Some(VreType::Galaxy),
        "oscar" => Some(VreType::Oscar),
        "vip" => Some(VreType::Vip),
        "boutique" => Some(VreType::Vip),
        "scipion" => Some(VreType::Scipion),
        "jupyter" => Some(VreType::Jupyter),
        "mddash" => Some(VreType::Mddash),
        "sciencemesh" => Some(VreType::Sciencemesh),
        "cernbox" => Some(VreType::Sciencemesh),
        "mybinder" => Some(VreType::Binder),
        "binder-launcher" => Some(VreType::Binder),
        "rrp" => Some(VreType::Rrp),
        _ => None,
    }
}

pub fn resolve_vre_type(tool: &ToolMeta) -> Result<VreType, String> {
    // First: explicit raw_definition["vre_type"]
    if let Some(value) = tool.raw_definition.get("vre_type") {
        if let Some(value) = value.as_str() {
            if let Some(vre_type) = VreType::from_str(value) {
                return Ok(vre_type);
            }
        }
    }

    // Second: tool types
    for tool_type in &tool.types {
        if let Some(vre_type) = tool_type_to_vre_type(tool_type) {
            return Ok(vre_type);
        }
    }

    // Third: URI pattern
    let patterns = [
        ("galaxyproject.org", VreType::Galaxy),
        ("usegalaxy.eu", VreType::Galaxy),
        ("usegalaxy.org", VreType::Galaxy),
        ("jupyter.org", VreType::Jupyter),
        ("oscar.grycap", VreType::Oscar),
        ("vip.creatis", VreType::Vip),
        ("cernbox.cern.ch", VreType::Sciencemesh),
        ("rrp-eosc", VreType::Rrp),
    ];

    for (pattern, vre_type) in patterns {
        if tool.uri.contains(pattern) {
            return Ok(vre_type);
        }
    }

    Err(format!("Cannot resolve vre_type from tool: {}", tool.id))
}

// Helpers
fn file_id(file: &FileEntry) -> String {
    file.download_url
        .clone()
        .unwrap_or_else(|| file.path.clone())
}

fn infer_encoding_format(url: &str) -> Option<&'static str> {
    let path = url.split('?').next().unwrap_or(url);
    let extension = path
        .rsplit('.')
        .next()
        .filter(|ext| !ext.contains('/'))?
        .to_ascii_lowercase();

    match extension.as_str() {
        "ipynb" => Some("application/x-ipynb+json"),
        "py" => Some("text/x-python"),
        "csv" => Some("text/csv"),
        "json" => Some("application/json"),
        "fastq" => Some("application/fastq"),
        "txt" => Some("text/plain"),
        "sh" => Some("text/x-shellscript"),
        "ga" => Some("application/galaxy"),
        "tiff" | "tif" => Some("image/tiff"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}

fn extract_filename_from_url(url: &str) -> String {
    let path = url.split('?').next().unwrap_or(url);

    path.rsplit('/').next().unwrap_or(path).to_string()
}

// RO-Crate builder
pub struct RocrateBuilder<'a> {
    request: &'a VreLaunchRequest,

    vre_type: VreType,
    programming_language: &'static str,
    display_name: &'static str,
    language_url: &'static str,
    lang_id: String,
    now_iso: String,
    graph: Vec<Value>,
}

impl<'a> RocrateBuilder<'a> {
    pub fn new(request: &'a VreLaunchRequest) -> Result<Self, String> {
        let vre_type = resolve_vre_type(&request.tool)?;

        Ok(Self {
            request,

            vre_type,
            programming_language: vre_type.programming_language(),
            display_name: vre_type.display_name(),
            language_url: vre_type.language_url(),

            lang_id: format!(
                "#{}-lang",
                match vre_type {
                    VreType::Galaxy => "galaxy",
                    VreType::Oscar => "oscar",
                    VreType::Scipion => "scipion",
                    VreType::Binder => "binder",
                    VreType::Jupyter => "jupyter",
                    VreType::Vip => "vip",
                    VreType::Mddash => "mddash",
                    VreType::Sciencemesh => "sciencemesh",
                    VreType::Rrp => "rrp",
                }
            ),

            now_iso: chrono::Utc::now().to_rfc3339(),

            graph: Vec::new(),
        })
    }

    // Metadata descriptor
    fn add_metadata_descriptor(&mut self) {
        self.graph.push(json!({
            "@id": "ro-crate-metadata.json",
            "@type": "CreativeWork",
            "about": {
                "@id": "./"
            },
            "conformsTo": {
                "@id": "https://w3id.org/ro/crate/1.1"
            }
        }));
    }

    // Root dataset, this is not the "dataset" in the sense of passed input.
    // But the ro-crate itself as a dataset.
    fn add_root_dataset(&mut self) {
        let mut has_part = Vec::new();

        // Tool
        has_part.push(json!({
            "@id": self.request.tool.uri
        }));

        // Additional files
        for slot in self.request.input.slots.values() {
            match slot {
                SlotValue::Value(_) => {
                    // NOTE(jyu): do nothing with value here, and the value is used in `add_workflow_entity`.
                    // This is a sign that the schema is overly complicate.
                }
                SlotValue::File(file_entry) => {
                    has_part.push(json!({
                        "@id": file_id(file_entry)
                    }));
                }
            }
        }

        // Dataset
        self.graph.push(json!({
            "@id": "./",
            "@type": "Dataset",
            "name": format!("Root dataset for tool: {}", self.request.tool.name),
            // // XXX: tool has description but
            // vre-crate didn't use it but set it to N/A
            // "description": &self.request.tool.description,
            "description": "N/A".to_string(),
            "datePublished": self.now_iso,
            "license": {
                "@id": "#license-unspecified"
            },
            "creator": {
                "@id": "#author-dispatcher"
            },
            "mainEntity": {
                "@id": self.request.tool.uri
            },
            "hasPart": has_part
        }));
    }

    // Workflow
    fn add_workflow_entity(&mut self) {
        let tool = &self.request.tool;

        let mut workflow_types = vec![json!("SoftwareSourceCode"), json!("ComputationalWorkflow")];

        let encoding_format = infer_encoding_format(&tool.uri);
        if encoding_format.is_some() {
            workflow_types.insert(0, json!("File"));
        }

        let name = if tool.name.is_empty() {
            extract_filename_from_url(&tool.uri)
        } else {
            tool.name.clone()
        };

        let description = if tool.description.is_empty() {
            "placeholder".to_string()
        } else {
            tool.description.clone()
        };

        let runtime_platform = self
            .request
            .runtime_platform
            .clone()
            .unwrap_or_else(|| self.vre_type.default_runtime_platform().to_string());

        let mut entity = json!({
            "@id": tool.uri,
            "@type": workflow_types,
            "conformsTo": {
                "@id":
                    "https://bioschemas.org/profiles/ComputationalWorkflow/0.5-DRAFT-2020_07_21/"
            },
            "name": name,
            "description": description,
            "programmingLanguage": {
                "@id": self.lang_id
            },
            "creator": {
                "@id": "#author-dispatcher"
            },
            "dateCreated":
                chrono::Utc::now()
                    .date_naive()
                    .to_string(),
            "license": {
                "@id": "#license-unspecified"
            },
            "sdPublisher": {
                "@id": "#workflow-hub"
            },
            "version": tool.version,
            "runtimePlatform": runtime_platform,
        });

        if let Some(encoding_format) = encoding_format {
            entity["encodingFormat"] = json!(encoding_format);
        }

        // NOTE(jyu): here is to create the like name.
        let input_refs: Vec<Value> = tool
            .slots
            .iter()
            .map(|slot| {
                json!({
                    "@id": format!("#input-{}", slot.id)
                })
            })
            .collect();

        if !input_refs.is_empty() {
            entity["input"] = json!(input_refs);
        }

        self.graph.push(entity);
    }

    // Programming language
    fn add_programming_language(&mut self) {
        self.graph.push(json!({
            "@id": self.lang_id,
            "@type": "ComputerLanguage",
            "identifier": self.programming_language,
            "name": self.display_name,
            "url": self.language_url
        }));
    }

    // Files
    fn add_slot_entries(&mut self) {
        for slot in &self.request.tool.slots {
            match slot.slot_typ {
                SlotTyp::Num | SlotTyp::Flag | SlotTyp::Str => {
                    if let SlotValue::Value(value) = self
                        .request
                        .input
                        .slots
                        .get(&slot.name)
                        .expect("not able to find the slot name")
                    {
                        let mut entity = json!({
                            "@id": format!("#input-{}", slot.id),
                            "@type": "FormalParameter",
                            "name": slot.name,
                            "additionalType": slot.slot_typ,
                            "required": !slot.is_optional,
                        });

                        entity["defaultValue"] = value.clone();

                        self.graph.push(entity);
                    }
                }
                SlotTyp::File => {
                    if let SlotValue::File(file_entry) = self
                        .request
                        .input
                        .slots
                        .get(&slot.name)
                        .expect("not able to find the slot name")
                    {
                        let mut entity = json!({
                            "@id": file_id(file_entry),
                            "@type": "File",
                            "name": file_entry.path,
                            "license": {
                                "@id": "#licens-unspecified"
                            }
                        });

                        if let Some(mime_type) = &file_entry.mime_type {
                            entity["encodingFormat"] = json!(mime_type);
                        }

                        if let Some(url) = &file_entry.download_url {
                            entity["url"] = json!(url);
                        }

                        entity["contentSize"] = json!(file_entry.size_bytes);

                        // FIXME: (jyu) the file entry not store the checksum type information.
                        if let Some(checksum) = &file_entry.checksum {
                            entity["sha256"] = json!(checksum);
                        }

                        // XXX: onedata is not specially treated at dispatcher. There for it is not needed to
                        // generate the payload with those fields.
                        //
                        // if let Some(domain) = &file.onedata_domain {
                        //     entity["onedata:onezoneDomain"] = json!(domain);
                        // }
                        //
                        // if let Some(file_id) = &file.onedata_file_id {
                        //     entity["onedata:fileId"] = json!(file_id);
                        // }

                        self.graph.push(entity);
                    }
                }
            }
        }
    }

    // Formal parameters
    fn add_formal_parameters(&mut self) {}

    // Dataset entity
    fn add_dataset_entity(&mut self) {
        let dataset = &self.request.input.dataset;
        self.graph.push(json!({
            "@id": dataset.url,
            "@type": "Dataset",
            "name": dataset.title,
            "description": dataset.description
        }));
    }

    // Tool metadata
    fn add_tool_metadata_entity(&mut self) {
        let raw_definition = &self.request.tool.raw_definition;
        if raw_definition.is_null() || raw_definition.as_object().is_some_and(|obj| obj.is_empty())
        {
            return;
        }

        self.graph.push(json!({
            "@id": "#tool-metadata",
            "@type": "Thing",
            "rawDefinition": self.request.tool.raw_definition
        }));
    }

    // Supporting entities
    fn add_supporting_entities(&mut self) {
        self.graph.push(json!({
            "@id": "#author-dispatcher",
            "@type": "Person",
            "name": "Dispatcher System"
        }));

        self.graph.push(json!({
            "@id": "#workflow-hub",
            "@type": "Organization",
            "name": "Example Workflow Hub",
            "url": "http://example.com/workflows/"
        }));

        self.graph.push(json!({
            "@id": "#license-unspecified",
            "@type": "CreativeWork",
            "name": "Unspecified license",
            "description": "License not specified by the crate producer",
        }));
    }

    // Build
    pub fn build(mut self) -> Value {
        self.add_metadata_descriptor();
        self.add_root_dataset();
        self.add_workflow_entity();
        self.add_programming_language();
        self.add_slot_entries();
        // XXX (jyu): All example in the vre-crate doesn't pass dataset from matchmaker, the example will be
        // used for mybinder tool.
        // self.add_dataset_entity();
        self.add_tool_metadata_entity();
        self.add_supporting_entities();

        json!({
            "@context": "https://w3id.org/ro/crate/1.1/context",
            "@graph": self.graph
        })
    }
}

// Public convenience function
pub fn build_from_launch_request(request: &VreLaunchRequest) -> Result<Value, String> {
    Ok(RocrateBuilder::new(request)?.build())
}

// Example
#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use crate::{DatasetHandle, Slot, ToolKind};

    use super::*;

    #[test]
    fn builds_ro_crate_vip() {
        let request = VreLaunchRequest {
            tool: ToolMeta {
                id: "cquest-pipeline".to_string(),
                version: "0.6".to_string(),
                name: "CQUEST Pipeline".to_string(),
                uri: "https://vip.creatis.insa-lyon.fr/rest/pipelines/CQUEST/0.6".to_string(),
                types: vec!["boutique".to_string(), "vip".to_string()],
                description: "An example workflow".to_string(),

                kind: ToolKind::SlotsAndFiles,
                slots: vec![
                    Slot {
                        id: "parameter_file".to_string(),
                        name: "parameter_file".to_string(),
                        slot_typ: SlotTyp::File,
                        is_optional: false,
                    },
                    Slot {
                        id: "data_file".to_string(),
                        name: "data_file".to_string(),
                        slot_typ: SlotTyp::File,
                        is_optional: false,
                    },
                    Slot {
                        id: "zipped_folder".to_string(),
                        name: "zipped_folder".to_string(),
                        slot_typ: SlotTyp::File,
                        is_optional: false,
                    },
                ],

                raw_definition: json!({}),
            },

            input: LaunchInput {
                dataset: DatasetHandle {
                    url: "https://example.org/dataset".to_string(),
                    title: "Example Dataset".to_string(),
                    description: "An example dataset".to_string(),
                },
                slots: HashMap::from([
                    (
                        "parameter_file".to_string(),
                        SlotValue::File(FileEntry {
                            path: "parameter_file".to_string(),
                            download_url: Some(
                                "https://www.creatis.insa-lyon.fr/~abonnet/quest_param_117T_A.txt"
                                    .to_string(),
                            ),
                            size_bytes: 1234,
                            mime_type: Some("text/csv".to_string()),
                            checksum: Some("abcdef123456".to_string()),
                            is_dir: false,
                            modified_at: DateTime::from_timestamp_nanos(323),
                        }),
                    ),
                    (
                        "data_file".to_string(),
                        SlotValue::File(FileEntry {
                            path: "./".to_string(),
                            download_url: Some(
                                "https://www.creatis.insa-lyon.fr/~abonnet/Rec003_Vox1.mrui"
                                    .to_string(),
                            ),
                            size_bytes: 1234,
                            mime_type: Some("text/csv".to_string()),
                            checksum: Some("abcdef123456".to_string()),
                            is_dir: false,
                            modified_at: DateTime::from_timestamp_nanos(323),
                        }),
                    ),
                    (
                        "zipped_folder".to_string(),
                        SlotValue::File(FileEntry {
                            path: "./".to_string(),
                            download_url: Some("https://example.org/input.csv".to_string()),
                            size_bytes: 1234,
                            mime_type: Some("text/csv".to_string()),
                            checksum: Some("abcdef123456".to_string()),
                            is_dir: false,
                            modified_at: DateTime::from_timestamp_nanos(323),
                        }),
                    ),
                ]),

                files: HashMap::new(),
            },

            runtime_platform: None,
        };

        let result = build_from_launch_request(&request).unwrap();

        println!("{}", serde_json::to_string_pretty(&result).unwrap());

        assert_eq!(result["@context"], "https://w3id.org/ro/crate/1.1/context");

        assert!(result["@graph"].is_array());
    }
}
