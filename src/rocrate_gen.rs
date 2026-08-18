/* 
* This file is align with the `rocrate.py` of vre_rocrate library.
*/
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

// Input models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetHandle {
    pub url: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlotDefinition {
    pub id: String,
    pub name: String,
    pub slot_type: String,
    pub is_optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInput {
    pub name: String,

    #[serde(default)]
    pub path: Option<String>,

    #[serde(default)]
    pub url: Option<String>,

    #[serde(default)]
    pub size_bytes: Option<u64>,

    #[serde(default)]
    pub mime_type: Option<String>,

    #[serde(default)]
    pub checksum: Option<String>,

    #[serde(default)]
    pub checksum_type: Option<String>,

    #[serde(default)]
    pub onedata_domain: Option<String>,

    #[serde(default)]
    pub onedata_file_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotValue {
    #[serde(default)]
    pub value: Option<Value>,

    #[serde(default)]
    pub file: Option<FileInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    pub id: String,
    pub version: String,
    pub name: String,
    pub uri: String,
    pub types: Vec<String>,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub slots: Vec<SlotDefinition>,

    #[serde(default)]
    pub raw_definition: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchInput {
    #[serde(default)]
    pub dataset: Option<DatasetHandle>,

    #[serde(default)]
    pub slots: HashMap<String, SlotValue>,

    #[serde(default)]
    pub files: HashMap<String, FileInput>,
}

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
fn file_id(file: &FileInput) -> String {
    file.url.clone().unwrap_or_else(|| file.name.clone())
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

    // Runtime platform
    fn runtime_platform(&self) -> String {
        self.request
            .runtime_platform
            .clone()
            .unwrap_or_else(|| self.vre_type.default_runtime_platform().to_string())
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

    // Root dataset
    fn add_root_dataset(&mut self) {
        let dataset = self.request.input.dataset.as_ref();

        let name = dataset
            .map(|d| d.title.clone())
            .unwrap_or_else(|| self.request.tool.name.clone());

        let description = dataset
            .map(|d| d.description.clone())
            .unwrap_or_else(|| self.request.tool.description.clone());

        let mut has_part = Vec::new();

        // Tool
        has_part.push(json!({
            "@id": self.request.tool.uri
        }));

        // Files attached to slots
        for slot_value in self.request.input.slots.values() {
            if let Some(file) = &slot_value.file {
                has_part.push(json!({
                    "@id": file_id(file)
                }));
            }
        }

        // Additional files
        for file in self.request.input.files.values() {
            has_part.push(json!({
                "@id": file_id(file)
            }));
        }

        // Dataset
        if let Some(dataset) = dataset {
            has_part.push(json!({
                "@id": dataset.url
            }));
        }

        self.graph.push(json!({
            "@id": "./",
            "@type": "Dataset",
            "name": name,
            "description": description,
            "datePublished": self.now_iso,
            "license": {
                "@id": "https://spdx.org/licenses/GPL-3.0"
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

        let encoding_format = infer_encoding_format(&tool.uri);

        let mut workflow_types = vec![json!("SoftwareSourceCode"), json!("ComputationalWorkflow")];

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
                "@id": "https://spdx.org/licenses/GPL-3.0"
            },
            "sdPublisher": {
                "@id": "#workflow-hub"
            },
            "version": tool.version,
            "runtimePlatform": self.runtime_platform(),
        });

        if let Some(encoding_format) = encoding_format {
            entity["encodingFormat"] = json!(encoding_format);
        }

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

    // File entity
    fn build_file_entity(&self, file: &FileInput) -> Value {
        let mut entity = json!({
            "@id": file_id(file),
            "@type": "File",
            "name": file.name,
            "license": {
                "@id": "https://spdx.org/licenses/GPL-3.0"
            }
        });

        if let Some(mime_type) = &file.mime_type {
            entity["encodingFormat"] = json!(mime_type);
        }

        if let Some(url) = &file.url {
            entity["url"] = json!(url);
        }

        if let Some(size_bytes) = file.size_bytes {
            entity["contentSize"] = json!(size_bytes);
        }

        if file.checksum_type.as_deref() == Some("sha256") {
            if let Some(checksum) = &file.checksum {
                entity["sha256"] = json!(checksum);
            }
        }

        if let Some(domain) = &file.onedata_domain {
            entity["onedata:onezoneDomain"] = json!(domain);
        }

        if let Some(file_id) = &file.onedata_file_id {
            entity["onedata:fileId"] = json!(file_id);
        }

        entity
    }

    // Files
    fn add_file_entities(&mut self) {
        for slot_value in self.request.input.slots.values() {
            if let Some(file) = &slot_value.file {
                self.graph.push(self.build_file_entity(file));
            }
        }

        for file in self.request.input.files.values() {
            self.graph.push(self.build_file_entity(file));
        }
    }

    // Formal parameters
    fn add_formal_parameters(&mut self) {
        for slot in &self.request.tool.slots {
            let mut entity = json!({
                "@id": format!("#input-{}", slot.id),
                "@type": "FormalParameter",
                "name": slot.name,
                "additionalType": slot.slot_type,
                "required": !slot.is_optional,
            });

            if let Some(slot_value) = self.request.input.slots.get(&slot.name) {
                if let Some(file) = &slot_value.file {
                    entity["defaultValue"] = json!({
                        "@id": file_id(file)
                    });
                } else if let Some(value) = &slot_value.value {
                    entity["defaultValue"] = value.clone();
                }
            }

            self.graph.push(entity);
        }
    }

    // Dataset entity
    fn add_dataset_entity(&mut self) {
        let Some(dataset) = &self.request.input.dataset else {
            return;
        };

        self.graph.push(json!({
            "@id": dataset.url,
            "@type": "Dataset",
            "name": dataset.title,
            "description": dataset.description
        }));
    }

    // Tool metadata
    fn add_tool_metadata_entity(&mut self) {
        if self.request.tool.raw_definition.is_empty() {
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
            "@id": "https://spdx.org/licenses/GPL-3.0",
            "@type": "CreativeWork",
            "name": "GNU General Public License v3.0",
            "alternateName": "GPL-3.0"
        }));
    }

    // Build
    pub fn build(mut self) -> Value {
        self.add_metadata_descriptor();
        self.add_root_dataset();
        self.add_workflow_entity();
        self.add_programming_language();
        self.add_file_entities();
        self.add_formal_parameters();
        self.add_dataset_entity();
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
    use super::*;

    #[test]
    fn builds_ro_crate() {
        let request = VreLaunchRequest {
            tool: ToolMeta {
                id: "example-tool".to_string(),
                version: "1.0".to_string(),
                name: "Example Workflow".to_string(),
                uri: "https://example.org/workflow.py".to_string(),
                types: vec!["jupyter".to_string()],
                description: "An example workflow".to_string(),

                slots: vec![SlotDefinition {
                    id: "input".to_string(),
                    name: "input".to_string(),
                    slot_type: "file".to_string(),
                    is_optional: false,
                }],

                raw_definition: serde_json::Map::new(),
            },

            input: LaunchInput {
                dataset: Some(DatasetHandle {
                    url: "https://example.org/dataset".to_string(),
                    title: "Example Dataset".to_string(),
                    description: "An example dataset".to_string(),
                }),

                slots: HashMap::from([(
                    "input".to_string(),
                    SlotValue {
                        value: None,
                        file: Some(FileInput {
                            name: "input.csv".to_string(),
                            path: None,
                            url: Some("https://example.org/input.csv".to_string()),
                            size_bytes: Some(1234),
                            mime_type: Some("text/csv".to_string()),
                            checksum: Some("abcdef123456".to_string()),
                            checksum_type: Some("sha256".to_string()),
                            onedata_domain: None,
                            onedata_file_id: None,
                        }),
                    },
                )]),

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
