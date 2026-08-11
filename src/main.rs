use chrono::{DateTime, Utc};
use datahugger::{
    crawl,
    crawler::{CrawlerError, ProgressManager},
    resolve, resolve_doi_to_url, Entry,
};
use exn::Exn;
use futures::stream;
use futures_core::stream::BoxStream;
use futures_util::StreamExt;
use indicatif::ProgressBar;
use req_packager::{
    grpc::{
        dataplayer_service_server::DataplayerServiceServer,
        dataset_service_server::DatasetServiceServer, tool_service_server::ToolServiceServer,
    },
    Artifact, AuthToken, Claims, DataRelayer, DataSource, Dataplayer, DatasetInfo, Dispatcher,
    FileEntry, HandlerId, LaunchInput, RawToken, RenameName, Slot, SlotValue, TaskHandler,
    ToolDatabase, ToolKind, ToolMeta, ToolSource, ToolState, UserId, UserInfo,
};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT},
    Client, ClientBuilder,
};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use sqlx::{types::time::OffsetDateTime, PgPool};
use tonic_health::server::HealthReporter;

use tokio::sync::RwLock;
use uuid::Uuid;

use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tonic::transport::Server;
use url::Url;

struct DatahuggerDataSource {
    pool: PgPool,
}

impl DatahuggerDataSource {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }
}

trait CrawlFileExt {
    fn crawl_file(
        self,
        client: &Client,
        mp: impl ProgressManager,
    ) -> BoxStream<'static, Result<FileEntry, Exn<CrawlerError>>>;
}

impl CrawlFileExt for datahugger::Dataset {
    fn crawl_file(
        self,
        client: &Client,
        mp: impl ProgressManager,
    ) -> BoxStream<'static, Result<FileEntry, Exn<CrawlerError>>> {
        let root_dir = self.root_dir();
        let client = ClientWithMiddleware::new(client.clone(), []);
        crawl(client, Arc::clone(&self.backend), root_dir, mp.clone())
            .filter_map(|res| async move {
                match res {
                    // TODO: need dir as well for the layout in the UI.
                    Ok(Entry::Dir(_)) => None,
                    Ok(Entry::File(f)) => {
                        let f: FileEntry = f.into();
                        Some(Ok(f))
                    }
                    Ok(Entry::Zip(_)) => None,
                    Err(e) => Some(Err(e)),
                }
            })
            .boxed()
    }
}

#[derive(Clone)]
struct NoProgress;

impl ProgressManager for NoProgress {
    fn insert(&self, _index: usize, _pb: ProgressBar) -> ProgressBar {
        ProgressBar::hidden()
    }

    fn insert_from_back(&self, _index: usize, _pb: ProgressBar) -> ProgressBar {
        ProgressBar::hidden()
    }
}

#[async_trait::async_trait]
impl DataSource for DatahuggerDataSource {
    async fn get_dataset_info(&self, uuid: &str) -> anyhow::Result<DatasetInfo> {
        let url = uuid;
        let info = DatasetInfo {
            uuid: Uuid::new_v4(),
            url: url.to_string(),
            id: "dummy".to_string(),
            description: "datahugger not yet support dataset metadata harvesting".to_string(),
            total_files: None,
            total_size_bytes: None,
            created_at: None,
            updated_at: None,
            tags: HashMap::new(),
        };
        Ok(info)
    }

    async fn list_files(&self, uuid: &str) -> anyhow::Result<BoxStream<'static, FileEntry>> {
        // `select * from record_files rf where record_identifier = '10.17026/AR/0KCPYB' order by rf.file_type desc`
        let user_agent = format!(
            "datahugger-over-eosc-coordinator/{}",
            env!("CARGO_PKG_VERSION")
        );
        let mut headers = HeaderMap::new();
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("token {token}"))?,
            );
        }
        if let Ok(token) = std::env::var("DRYAD_API_TOKEN") {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))?,
            );
        }
        headers.insert(USER_AGENT, HeaderValue::from_str(&user_agent)?);
        let client = ClientBuilder::new()
            .user_agent(user_agent)
            .default_headers(headers)
            .use_native_tls()
            .build()?;
        let mut url = uuid.to_string();
        if url.starts_with("https://doi.org/") {
            let doi = url.trim_start_matches("https://doi.org/");
            url = resolve_doi_to_url(&client, doi, true)
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        }
        #[derive(Debug)]
        struct RecordFile {
            download_url: String,
            file_type: Option<String>,
            file_size: Option<i64>,
            checksum_type: Option<String>,
            checksum_value: Option<String>,
            updated_at: OffsetDateTime,
        }
        match resolve(&url).await {
            Ok(ds) => {
                let mp = NoProgress;
                let files = ds
                    .crawl_file(&client, mp)
                    // TODO: I need log on error cases on the server.
                    .filter_map(|f| async move { f.ok() })
                    .boxed();
                Ok(files)
            }
            // NOTE: (jyu) fallback to filedb, this should revert with the datahugger fetch.
            // Should go to fileDB first and then goes to datahugger for the latest update.
            Err(err) => {
                eprintln!("resolve failed, fallback to DB: {:?}", err);

                let rows = sqlx::query_as!(
                    RecordFile,
                    r#"
                    SELECT 
                        download_url, 
                        file_type, 
                        file_size, 
                        checksum_type::text as "checksum_type?",
                        checksum_value,
                        updated_at
                    FROM record_files
                    WHERE record_identifier = $1
                    "#,
                    uuid
                )
                .fetch_all(&self.pool)
                .await?;

                // Convert DB rows
                let items: Vec<FileEntry> = rows
                    .into_iter()
                    .map(|r| FileEntry {
                        download_url: Some(r.download_url.clone()),
                        path: r.download_url, // XXX: no path stored
                        is_dir: false,
                        size_bytes: r.file_size.unwrap_or(0) as u64,
                        mime_type: r.file_type,
                        checksum: r.checksum_value,
                        // XXX: should make the name consistent
                        modified_at: DateTime::<Utc>::from_timestamp(
                            r.updated_at.unix_timestamp(),
                            r.updated_at.nanosecond(),
                        )
                        .unwrap(),
                    })
                    .collect();

                Ok(stream::iter(items).boxed())
            }
        }
    }
}

/// init with the root_api url, must be an valid url, to the version.
/// A valid example is:
/// http://tool-registry.eosc-data-commons.dansdemo.nl/api/v1/
pub struct ToolRegistry {
    root_api: Url,
}

impl ToolRegistry {
    fn new(root_api: Url) -> Self {
        ToolRegistry { root_api }
    }
}

// This is the type for the obj get from tool registry.
// Need to map to the Slot of inner representation.
#[derive(Deserialize, Debug)]
struct ResponseSlot {
    id: String,
    name: String,
    #[serde(rename = "type")]
    slot_type: String,
    optional: bool,
    // TODO: file_formats: Vec<String>,
}

impl From<ResponseSlot> for Slot {
    fn from(value: ResponseSlot) -> Self {
        Slot {
            id: value.id,
            slot_type: value.slot_type,
            name: value.name,
            is_optional: value.optional,
        }
    }
}

// This is the type for handle the API call return form api/tools/{id}
#[derive(Deserialize, Debug)]
struct OneToolPinResponse {
    // id: u64,
    uri: String,
    name: String,
    description: String,
    types: Vec<String>,
    version: String,
    input_slots: Vec<ResponseSlot>,
    raw_definition: JsonValue,
}

// This is the type for handle the API call return form api/search/ and api/match
// NOTE: (jyu) this should revisit to align with the schema: https://github.com/EOSC-Data-Commons/toolmeta-models/blob/main/src/toolmeta_models/tool_generic.py
#[derive(Deserialize, Debug)]
struct OneToolSearchResponse {
    id: u64,
    uri: String,
    name: String,
    description: String,
    types: Vec<String>,
    version: String,
    input_slots: Option<Vec<ResponseSlot>>,
}

static TOOLS: LazyLock<Vec<ToolMeta>> = LazyLock::new(|| {
    vec![
        ToolMeta {
            id: "::st:001".to_string(),
            version: "v0.1.3".to_string(),
            name: "EOSC-Data-Commons/binder-python-tool".to_string(),
            uri: "https://github.com/EOSC-Data-Commons/binder-python-tool".to_string(),
            types: vec!["general".to_string(), "egi-replay".to_string()],
            description: "binder python tool in egi-replay".to_string(),
            slots: vec![],
            kind: ToolKind::DatasetOnly,
            raw_definition: json!({
                "urlpath": "notebooks/python.ipynb"
            }
            ),
        },
        ToolMeta {
            id: "::st:002".to_string(),
            version: "v0".to_string(),
            name: "Reproduciple Research Platform (RRP)".to_string(),
            uri: "https://rrp-eosc.ethz.ch/".to_string(),
            types: vec!["general".to_string(), "rrp".to_string()],
            description: "RRP as genenal tool".to_string(),
            slots: vec![
                Slot {
                    id: "image_0.tif".to_string(),
                    name: "Image 0 (TIF)".to_string(),
                    slot_type: "file".to_string(),
                    is_optional: false,
                },
                Slot {
                    id: "image_1.tif".to_string(),
                    name: "Image 1 (TIF)".to_string(),
                    slot_type: "file".to_string(),
                    is_optional: false,
                },
            ],
            kind: ToolKind::DatasetOnly,
            raw_definition: json!({
                "repositoryUrl": "https://gitlab.ethz.ch/Reproducible-Research-Platform/tools/Cell-Doubling-Time",
                "docker_image": "reproducibleresearchplatform/rrp-eosc:cell-doubling-time_1.0.1"
            }),
        },
        ToolMeta {
            id: "::st:003".to_string(),
            version: "v0".to_string(),
            name: "CernBox".to_string(),
            uri: "cernbox.cern.ch".to_string(),
            types: vec!["data access".to_string(), "cernbox".to_string()],
            description: "Tool to send files to CernBox user".to_string(),
            slots: vec![Slot {
                id: "shared_with".to_string(),
                name: "Shared With".to_string(),
                slot_type: "string".to_string(),
                is_optional: false,
            }],
            kind: ToolKind::SlotsAndFiles,
            raw_definition: json!({}),
        },
    ]
});

#[async_trait::async_trait]
impl ToolSource for ToolRegistry {
    async fn search_tools_by_text(&self, text: &str) -> anyhow::Result<Vec<ToolMeta>> {
        if text.starts_with("::STATIC") {
            let tools = &TOOLS;
            return Ok(tools.to_vec());
        }
        // http://tool-registry.eosc-data-commons.dansdemo.nl/api/v1/tools/?name=OCR
        let url = format!("{}/tools/?name={}", self.root_api.as_str(), text);
        tracing::info!("url: {}", url);
        let resp = reqwest::get(url).await?;
        let resp: Vec<OneToolSearchResponse> = resp.json().await?;
        let tools = resp
            .into_iter()
            .map(|resp| {
                let slots = if let Some(input_slots) = resp.input_slots {
                    input_slots
                        .into_iter()
                        .map(|s| s.into())
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };

                let kind = if resp.types.contains(&"data access".to_string()) {
                    ToolKind::SlotsAndFiles
                } else {
                    ToolKind::SlotsOnly
                };
                ToolMeta {
                    id: resp.id.to_string(),
                    version: resp.version,
                    uri: resp.uri,
                    types: resp.types,
                    name: resp.name,
                    description: resp.description,
                    slots,
                    kind,
                    raw_definition: json!({}),
                }
            })
            .collect::<Vec<_>>();
        return Ok(tools);
    }

    async fn find_tools(&self, files: &[FileEntry]) -> anyhow::Result<Vec<ToolMeta>> {
        let client = reqwest::Client::new();

        #[derive(Serialize, Debug)]
        struct Input {
            name: String,
            mime_type: String,
        }

        #[derive(Serialize, Debug)]
        struct Options {
            operator: String,
        }

        #[derive(Serialize, Debug)]
        struct Payload {
            r#type: String,
            inputs: Vec<Input>,
            options: Options,
        }

        let inputs = files
            .iter()
            .map(|f| {
                let name = PathBuf::from_str(&f.path).unwrap();
                let name = name.file_name().unwrap().to_str().unwrap();
                Input {
                    name: name.to_string(),
                    mime_type: f.mime_type.clone().unwrap_or("unknown".to_string()),
                }
            })
            .collect();

        let payload = Payload {
            r#type: "file".to_string(),
            inputs,
            options: Options {
                operator: "or".to_string(),
            },
        };

        let url = format!("{}/tools/match", self.root_api.as_str());
        let response = client
            .post(url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let response: Vec<OneToolSearchResponse> = response.json().await.inspect_err(|err| {
            // dbg!(err);
        })?;
        let tools = response
            .into_iter()
            .map(|resp| {
                let slots = if let Some(input_slots) = resp.input_slots {
                    input_slots
                        .into_iter()
                        .map(|s| s.into())
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };

                let kind = if resp.types.contains(&"data access".to_string()) {
                    ToolKind::SlotsAndFiles
                } else {
                    ToolKind::SlotsOnly
                };
                ToolMeta {
                    id: resp.id.to_string(),
                    version: resp.version,
                    uri: resp.uri,
                    types: resp.types,
                    name: resp.name,
                    description: resp.description,
                    slots,
                    kind,
                    raw_definition: json!({}),
                }
            })
            .collect::<Vec<_>>();
        Ok(tools)
    }

    async fn get_tool(&self, id: &str) -> anyhow::Result<ToolMeta> {
        if id.starts_with("::st") {
            if let Some(tool) = TOOLS.to_vec().iter().find(|&t| t.id == id) {
                return Ok(tool.to_owned());
            }
        }
        let url = format!("{}/tools/{}", self.root_api.as_str(), id);
        let resp: OneToolPinResponse = reqwest::get(url).await?.json().await?;
        let slots = resp
            .input_slots
            .into_iter()
            .map(|s| s.into())
            .collect::<Vec<_>>();

        // NOTE: (jyu) need to document this so when new VRE onboarding it knows which type to set.
        let kind = if resp.types.contains(&"data access".to_string()) {
            ToolKind::SlotsAndFiles
        } else {
            ToolKind::SlotsOnly
        };
        let tool = ToolMeta {
            id: id.to_string(),
            version: resp.version,
            uri: resp.uri,
            types: resp.types,
            name: resp.name,
            description: resp.description,
            slots,
            kind,
            raw_definition: resp.raw_definition,
        };
        return Ok(tool);
    }
}

struct MockDispatcher {
    db: RwLock<HashMap<Uuid, TaskHandler>>,
}

impl MockDispatcher {
    fn new() -> Self {
        Self {
            db: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl Dispatcher for MockDispatcher {
    // // launch a vre with the launch request, return the callback url when it is ready
    async fn launch(
        &self,
        user_info: &UserInfo,
        token: &RawToken,
        tool: &ToolMeta,
        input: &LaunchInput,
        api_keys: &HashMap<String, String>,
    ) -> anyhow::Result<Uuid> {
        // it also relates to the auth problem, who has the access to the vre? who should control
        // the permission of vre. I think it should be the vre provider and somewhere there is a
        // mapping for what eosc user can access which vres. Should this all kept in an auth server
        // (assume it will be one), or dispatcher maintain the table and mapping??
        // thus non-mock one should take care of auth here or somewhere in front

        // in the mock, this will be just
        // 1. have a in-memory db (mocked by HashMap) to record user and tools launched
        // 2. return a dummy url to be printed in the UI frontend.

        // should tool meta contain all info to let dispatcher know "how to launch a tool?"

        // VIP
        // RRP
        //

        // XXX: mock only the galaxy behavior, @reggie we need to find the pattern here to do the proper
        // abstraction.
        //
        // POST https://usegalaxy.eu/api/workflow_landings
        // ```json
        // {
        //   "public": false,
        //   "workflow_id": "https://dockstore.org/api/ga4gh/trs/v2/tools/%23workflow%2Fgithub.com%2Flaitanawe%2Fismb2024%2Fgalaxy_example/versions/main/PLAIN_GALAXY/descriptor/Galaxy-Workflow-reverse_file_galaxy_workflow.ga", # trs is one of the ga4gh spec, defind the API, of it is a workflow or tool.
        //   "workflow_target_type": "trs_url", # trs standard
        //   "request_state": {
        //     "simpletext_input": {
        //       "class": "File",
        //       "filetype": "txt",
        //       "location": "https://example-files.online-convert.com/document/txt/example.txt"
        //     }
        //   }
        // }
        // ```
        // HTTP 200
        // [Asserts]
        // jsonpath "$.uuid" exists
        // [Captures]
        // landing_uuid: jsonpath "$.uuid"
        //
        // return this url
        // # GET https://usegalaxy.eu/workflow_landings/{{landing_uuid}}?public=false

        // NOTE: @reggie, I want ToolMeta include info says "I am a tool need to use galaxy as VRE
        // to launch me. Then in the realworld `launch` implementation it goes to dispatcher to
        // launch the galaxy with the specific information attached."
        //
        // struct ToolMeta {
        //     // id, version, name, description, slots are needed by the UI.
        //     id: String,
        //     version: String,
        //     name: String,
        //     description: String,
        //
        //     // XXX: !!! runtime is a VRE type which indicate how the tool need to be launched.
        //     // not very clear how the VRE specific information passed to here, because different
        //     // VRE has different information required, therefore the layout of input is dynamic.
        //     // `workflow_id` and `trs_url` are such kinds of information.
        //
        //     runtime: RuntimeMeta,
        // }
        //
        // struct RuntimeMeta {
        //     kind: RuntimeKind, // this not dynamically support adding new runtime
        //     config: serde_json::Value,
        // }
        //
        // enum RuntimeKind {
        //     Galaxy,
        //     RRP,
        //     VIP,
        // }
        //
        // fun foo(tool: ToolMeta) {
        //      match tool.runtime.kind {
        //          RuntimeKind::Galaxy {
        //              let cfg: GalaxyRuntime = serde_json::from_value(runtime.config)?;
        //              ...
        //          }
        //      }
        // }
        //
        // // json will be like
        // // {
        // //   "id": "toolid-706",
        // //   "runtime": {
        // //     "config": {
        // //       "workflow_id": "xxx",
        // //       "workflow_target_type": "trs_url",
        // //       "request_state": {
        // //         "simpletext_input": {
        // //           "class": "File",
        // //           "filetype": "txt",
        // //           "location": "https://example-files.online-convert.com/document/txt/example.txt"
        // //         }
        // //       }
        // //     }
        // //   }
        // // }
        //
        // proxy/plugin runs for every VRE in its own process and talk to dispatcher with a well
        // defined protocol.

        // TODO: tool slots type need to be validated here before send the final launch action.
        // This can also happens in the frontend to prevent user pass the wrong type.
        // libmagic (its ML support version) can be used to do file type validation beyond the
        // extension.

        // NOTE: the actual logic here should be:
        // 1. check the tool type, if it is a) from workflowhub and b) galaxy tool
        // 2. get the workflowhub ga4ph link.
        // 3. assemble the payload
        // 4. send the payload
        //
        // NOTE: There are two variable approaches:
        // 1. tool meta contains only the vre id, the vre payload in assemble by the specific
        //    service.
        // 2. tool meta contains runtime type (id to identify the vre again), but contain the
        //    config with known layout for VREs.
        //
        // jyu: approach (1) is more proper in production, but require dispatcher / or another
        // component play the role as "VRE" registry.
        //
        #[derive(Deserialize)]
        struct VersionResp {
            id: String,
            name: String,
        }

        let uid = &user_info.sub;

        if tool.types.contains(&"galaxy_workflow".to_string())
            && tool.types.contains(&"workflowhub".to_string())
        {
            let client = reqwest::Client::new();
            let workflow_id = tool.uri.split('/').next_back().unwrap();
            // XXX: @reggie, I need to make an extra call to get the latest version id, because
            // what stored in your tool registry response is the tag of the version.
            let res = client
                .get(format!(
                    "https://workflowhub.eu/ga4gh/trs/v2/tools/{}/versions",
                    workflow_id
                ))
                .send()
                .await?;

            let resp_versions: Vec<VersionResp> = res.json().await?;
            // XXX: sehr ugly
            let version = resp_versions
                .into_iter()
                .filter(|i| i.name == tool.version)
                .map(|i| i.id)
                .collect::<Vec<_>>();

            // NOTE: only launch the latest version
            // TODO: in tool registry, harvest all version and in matchmaker UI allow to select
            // versions.
            let workflow_id = format!(
                "https://workflowhub.eu/ga4gh/trs/v2/tools/{}/versions/{}",
                workflow_id, version[0],
            );

            let slots = &input.slots;

            let request_state: serde_json::Map<String, serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    match entry {
                        SlotValue::Value(_) => {
                            // FIXME: get values from rpc client
                            todo!()
                        }
                        SlotValue::File(f) => {
                            let location = f.download_url.as_deref()?;

                            let filetype = f.path.rsplit('.').next().unwrap_or("txt");

                            Some((
                                key.clone(),
                                // @reggie galaxy specific info
                                serde_json::json!({
                                    "class": "File",
                                    "filetype": filetype,
                                    "location": location
                                }),
                            ))
                        }
                    }
                })
                .collect();

            let payload = serde_json::json!({
                "public": false,
                "workflow_id": workflow_id,
                "workflow_target_type": "trs_url",
                "request_state": request_state,
            });

            #[derive(serde::Deserialize)]
            struct Response {
                uuid: String,
            }

            // XXX: this is a blocking call, blocking call should not stay in async block.
            // See if galaxy provide async call that return immediately with a handler to check the
            // state.
            let res = client
                .post("https://usegalaxy.eu/api/workflow_landings")
                .json(&payload)
                .send()
                .await?;

            let data: Response = res.json().await.unwrap();
            let landing_uuid = data.uuid;
            let callback_url = Url::from_str(&format!(
                "https://usegalaxy.eu/workflow_landings/{landing_uuid}?public=false"
            ))
            .expect("a valid url");

            let id = uuid::Uuid::new_v4();
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(id).or_insert(task_handler);

            Ok(id)
        } else if tool.types.contains(&"boutique".to_string())
            && tool.types.contains(&"vip".to_string())
        {
            // VIP case

            // this will be the task handle id stored in the dispatcher DB
            // and the id is also used in VIP job as the id in the job name.
            let task_id = uuid::Uuid::new_v4();

            let user_agent = format!("eosc-coordinator/{}", env!("CARGO_PKG_VERSION"));
            let mut headers = HeaderMap::new();
            if let Some(key) = api_keys.get("vip") {
                headers.insert("apikey", HeaderValue::from_str(&key.to_string())?);
                // dbg!(key);
            }
            let client = ClientBuilder::new()
                .user_agent(user_agent)
                .default_headers(headers)
                .use_native_tls()
                .build()?;

            // NOTE: this is the payload
            //
            // POST https://vip.creatis.insa-lyon.fr/test/rest/executions
            // apikey: {{VIP_API_KEY}}
            // ```json
            // {
            //     "name" : "test-http-with-api",
            //     "pipelineIdentifier":"CQUEST/0.6",
            //     "resultsLocation" : "/vip/Home",
            //     "inputValues" : {
            //         "parameter_file": "https://www.creatis.insa-lyon.fr/~abonnet/quest_param_117T_A.txt",
            //         "data_file": "https://www.creatis.insa-lyon.fr/~abonnet/Rec003_Vox1.mrui",
            //         "zipped_folder": "https://www.creatis.insa-lyon.fr/~abonnet/basis_11_7.zip"
            //     }
            // }
            // ```

            let slots = &input.slots;

            // TODO:
            let request_state: serde_json::Map<String, serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    let slot_id = tool
                        .slots
                        .iter()
                        .find(|s| s.name == *key)
                        .map(|s| s.id.clone())?;
                    match entry {
                        SlotValue::Value(v) => Some((slot_id, v.clone())),
                        SlotValue::File(f) => {
                            // VIP use the slot id as the key of the file list in the payload
                            let location = f.download_url.as_deref()?;

                            let v = serde_json::Value::String(location.to_string());
                            Some((slot_id, v))
                        }
                    }
                })
                .collect();

            let pipe_name = format!("{}/{}", tool.name, tool.version);

            let payload = serde_json::json!({
                "name": format!("eosc-{task_id}"),
                "pipelineIdentifier": pipe_name,
                "resultsLocation": "/vip/Home",
                "inputValues": request_state,
            });

            #[derive(serde::Deserialize)]
            struct Response {
                uuid: String,
            }

            // XXX: this is a blocking call, blocking call should not stay in async block.
            // See if galaxy provide async call that return immediately with a handler to check the
            // state.
            let _resp = client
                .post("https://vip.creatis.insa-lyon.fr/test/rest/executions")
                .json(&payload)
                .send()
                .await?;

            // XXX: here should propagate the error because the payload can be wrong and the job
            // cannot be start.
            // check the resp state and return the error, or return Uuid but make it directly as
            // failed??

            // TODO: the response can be used for state tracking

            // XXX: vip is redesign the ui, thus there will be a redirect link to the launched job.
            let callback_url =
                Url::from_str("https://vip.creatis.insa-lyon.fr/home.html").expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"mybinder".to_string()) {
            let task_id = uuid::Uuid::new_v4();
            // todo: need to use the helper function i provide in datahugger to get the branch or
            // commit number.
            let callback_url = Url::from_str(&input.dataset.url).expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"binder-launcher".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            let raw = &tool.raw_definition;

            // ---- required fields ----
            let binder_base = raw
                .get("binder_base")
                .and_then(|v| v.as_str())
                .unwrap_or("https://mybinder.org")
                .trim_end_matches('/');

            let launcher_repo = raw
                .get("launcher_repo")
                .and_then(|v| v.as_str())
                .expect("missing launcher_repo");

            let launcher_ref = raw
                .get("launcher_ref")
                .and_then(|v| v.as_str())
                .unwrap_or("main");

            let target_repo = raw
                .get("target_repo")
                .and_then(|v| v.as_str())
                .expect("missing target_repo");

            // optional
            let branch = raw.get("branch").and_then(|v| v.as_str());
            let notebook_path = raw.get("notebook_path").and_then(|v| v.as_str());
            let overwrite = raw
                .get("overwrite")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let cleanup = raw
                .get("cleanup")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let run_postbuild = raw
                .get("run_postbuild")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let inner_urlpath = {
                // inner query builder
                let mut inner = form_urlencoded::Serializer::new(String::new());

                inner.append_pair("repo", target_repo);

                if let Some(branch) = branch {
                    if !branch.is_empty() && branch != "main" {
                        inner.append_pair("branch", branch);
                    }
                }

                if let Some(path) = notebook_path {
                    if !path.is_empty() {
                        inner.append_pair("notebookpath", path);
                    }
                }

                if !overwrite {
                    inner.append_pair("overwrite", "0");
                }

                if cleanup {
                    inner.append_pair("cleanup", "1");
                }

                if run_postbuild {
                    inner.append_pair("run_postbuild", "1");
                }

                // ---- env ----
                if let Some(env) = raw.get("env").and_then(|v| v.as_object()) {
                    for (k, v) in env {
                        if let Some(val) = v.as_str() {
                            inner.append_pair(k, val);
                        }
                    }
                }

                // ---- data files from input.files ----
                let mut data_files_json = Vec::new();

                let files = &input.files;
                for (name, file) in files.iter() {
                    data_files_json.push(serde_json::json!({
                        "url": file.download_url,
                        "path": name.to_string()
                    }));
                }

                let dataset_url = &input.dataset.url;
                data_files_json.push(serde_json::json!({
                    "url": dataset_url,
                    "path": null
                }));

                if !data_files_json.is_empty() {
                    let json =
                        serde_json::to_string(&data_files_json).expect("serialize data_files");

                    inner.append_pair("data", &json);
                }
                format!("launch?{}", inner.finish())
            };

            // ---- outer URL ----
            let mut callback_url = Url::parse(&format!(
                "{}/v2/gh/{}/{}",
                binder_base, launcher_repo, launcher_ref
            ))
            .expect("valid base url");

            callback_url
                .query_pairs_mut()
                .append_pair("urlpath", &inner_urlpath);

            tracing::info!("Create project: {}", callback_url);

            // ---- task handler ----
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };

            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"egi-replay".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            // construct:
            // https://replay.notebooks.egi.eu/v2/gh/EOSC-Data-Commons/binder-python-tool/v0.1.1?urlpath=notebooks/python.ipynb?dataset_url=https://zenodo.org/records/20844503
            let replay_index = "https://replay.notebooks.egi.eu/v2/gh";
            // let tool_name = "EOSC-Data-Commons/binder-python-tool";
            let tool_name = &tool.name;
            // let version = "v0.1.1";
            let version = &tool.version;
            // let urlpath = "notebooks/python.ipynb";
            let urlpath = tool
                .raw_definition
                .get("urlpath")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");
            let urlpath = urlpath.to_string();
            // let dataset_url = "https://zenodo.org/records/20844503";
            let dataset_url = &input.dataset.url;

            let callback_url = format!(
                "{replay_index}/{tool_name}/{version}?urlpath={urlpath}?dataset_url={dataset_url}"
            );

            let callback_url = Url::from_str(&callback_url).expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"cernbox".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            let files = &input.files;
            let slots = &input.slots;

            let domain = "eosc.cernbox.cern.ch";
            let client = Client::builder().build()?;
            let Some(share_with) = slots.get("Shared With").map(|v| match v {
                SlotValue::File(_) => unreachable!("must be a value"),
                SlotValue::Value(v) => {
                    let serde_json::Value::String(v) = v else {
                        unreachable!("must be a string")
                    };
                    v
                }
            }) else {
                unreachable!("'Share With' must be set")
            };
            let share_with = format!("{share_with}@{domain}");

            // XXX: look at all fields here
            // ??, should name and description customized by user?
            let owner = &user_info.email;
            let email = &user_info.email;

            // TODO: (jyu) 'name' and 'preferred_username' is optinal, should I implement fallback logic?
            // let sender_display_name = &user_info.preferred_username;
            let sender_display_name = email.split('@').collect::<Vec<_>>()[0];

            // TODO: this needs to be constructed, and this is the main OCM trick.
            let sender = format!("{email}@eosc-coordinator.ethz.ch");

            fn create_rocrate(
                files: &HashMap<RenameName, FileEntry>,
                share_with: &str,
                domain: &str,
                title: &str,
            ) -> serde_json::Value {
                let mut graph: Vec<serde_json::Value> = Vec::new();
                let mut has_part: Vec<serde_json::Value> = Vec::new();

                // Take only first two files
                for (i, (name, file)) in files.iter().enumerate() {
                    let id = format!("#file-{}", i);

                    has_part.push(json!({ "@id": id }));

                    graph.push(json!({
                        "@id": id,
                        "@type": "File",
                        // XXX: this should be rename_to
                        "name": name.to_string(),
                        // "description": file.description, // ?? need this??
                        "encodingFormat": file.mime_type,
                        "url": &file.download_url
                    }));
                }

                // Root dataset
                graph.insert(0, json!({
                    "@id": "./",
                    "@type": "Dataset",
                    "name": title,
                    "description": "(yet not passed) A research data package with Jupyter notebook and datasets for sharing through ScienceMesh federation",
                    "datePublished": chrono::Utc::now().to_rfc3339(),
                    "creator": { "@id": "#creator" },
                    "runsOn": { "@id": "#destination" },
                    "hasPart": has_part
                }));

                // Metadata descriptor
                graph.push(json!({
                    "@id": "ro-crate-metadata.json",
                    "@type": "CreativeWork",
                    "about": { "@id": "./" },
                    "conformsTo": { "@id": "https://w3id.org/ro/crate/1.1" }
                }));

                // Static entities (keep your existing ones)
                graph.push(json!({
                    "@id": "#destination",
                    "@type": "Service",
                    "name": "ScienceMesh Service",
                    "url": format!("https://{domain}"),
                }));

                // XXX: redundant information, record twice.
                // The ro-crate format required by the cernbox is a subset of EDC ro-crate.
                graph.push(json!({
                    "@id": "#creator",
                    "@type": "Person",
                    "name": "TBD",
                    "userid": "TBD",
                }));

                graph.push(json!({
                    "@id": "#sender",
                    "@type": "Person",
                    "name": "TBD",
                    "userid": "TBD",
                }));

                graph.push(json!({
                    "@id": "#receiver",
                    "@type": "Person",
                    "userid": share_with,
                }));

                json!({
                    "@context": "https://w3id.org/ro/crate/1.1/context",
                    "@graph": graph
                })
            }

            let dataset_title = &input.dataset.title;
            let rocrate = create_rocrate(files, &share_with, domain, dataset_title);

            // ---- CREATE PROJECT ----
            let project_data = serde_json::json!({
                "shareWith": share_with,
                "name": dataset_title,
                // XXX: (jyu) not passed from launch tool call from upstream matchmaker
                "description": "",
                "providerId": &uuid::Uuid::new_v4(),
                "resourceId": task_id,
                "owner": owner,
                "senderDisplayName": sender_display_name,
                "sender": sender,
                "resourceType": "ro-crate",
                "shareType": "user",
                "protocol": {
                  "name": "multi",
                  "embedded": {"payload": rocrate}}
                }
            );

            println!("{}", serde_json::to_string_pretty(&project_data).unwrap());

            let api_url = format!("https://{domain}/ocm/shares");
            let resp = client.post(api_url).json(&project_data).send().await?;

            if !resp.status().is_success() {
                dbg!(&resp);
                dbg!("fail here");
                let artifact = Artifact::FailedTool;
                let task_handler = TaskHandler {
                    id: HandlerId(task_id),
                    user_id: UserId(uid.to_string()),
                    state: ToolState::Exception,
                    artifact,
                };

                let mut db = self.db.write().await;
                db.entry(task_id).or_insert(task_handler);

                return Ok(task_id);
            }

            let callback_url = Url::from_str(&format!("https://{domain}")).expect("valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"rrp".to_string()) {
            let task_id = uuid::Uuid::new_v4();
            let backend_url = "https://rrp-eosc.ethz.ch";
            let client = Client::builder().build()?;

            // let LaunchInput::FilesOnly(files) = input else {
            //     panic!("not possible.")
            // };
            let hdataset = &input.dataset;
            let doi = hdataset.url.trim_start_matches("https://doi.org/");
            // dbg!(doi);
            // doi = "10.5281/zenodo.20507550"

            // CREATE PROJECT
            let slots = &input.slots;
            let data_mounts: Vec<serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    let slot = tool.slots.iter().find(|s| s.name == *key)?;

                    match entry {
                        SlotValue::File(f) => {
                            let path = &f.path.trim_start_matches("__ROOT__/");

                            Some(serde_json::json!({
                                "mountPath": slot.id,
                                "source": {
                                    "type": "zenodo",
                                    "doi": doi
                                },
                                "path": path
                            }))
                        }
                        _ => None, // ignore non-files
                    }
                })
                .collect();

            let image = tool
                .raw_definition
                .get("docker_image")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");

            // FIXME: the image should coming from tool-metadata
            // FIXME: the descriptino should be the tool+dataset
            let project_data = serde_json::json!({
                "image": image,
                "name": format!("eosc-{task_id}"),
                "description": "Created via Coordinator",
                "resources": {
                    "cpu": 1.0,
                    "memMb": 2048
                },
                "dataMounts": data_mounts,
            });

            // // FIXME: start project and pass files into it
            // let Ok(oidc_agent_token) = std::env::var("OIDC_AGENT_TOKEN") else {
            //     panic!("oidc_agent_token not found in env var")
            // };
            let resp = client
                .post(format!("{}/external/dispatcher/v1/projects", backend_url))
                .bearer_auth(token)
                .json(&project_data)
                .send()
                .await?;

            if !resp.status().is_success() {
                dbg!("fail here");
                let artifact = Artifact::FailedTool;
                // TODO: use TaskHandler::new()
                let task_handler = TaskHandler {
                    id: HandlerId(task_id),
                    user_id: UserId(uid.to_string()),
                    state: ToolState::Exception,
                    artifact,
                };

                let mut db = self.db.write().await;
                db.entry(task_id).or_insert(task_handler);

                return Ok(task_id);
            }

            let location = resp
                .headers()
                .get("Location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            // NOTE: a rust excersize, why this moves??
            // let location = resp.headers().get("Location").and_then(|v| v.to_str().ok());

            tracing::info!("Create project: {}", resp.status());

            // Poll and wait until creationStatus == "Ready"
            let project_url = resp.json::<serde_json::Value>().await?["id"]
                .as_str()
                .expect("missing project id")
                .to_string();

            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project did not reach 'Ready' status after 20 attempts"
                    ));
                }

                attempts += 1;

                let status_resp = client.get(&project_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = status_resp.json().await?;
                let status = json["creationStatus"].as_str().unwrap_or("");

                if status == "Ready" {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait creation: attempts {}", attempts);
            }

            let repository_url = tool
                .raw_definition
                .get("repositoryUrl")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");

            // Clone repository (async)
            let clone_resp = client
                .post(format!("{}/clone", project_url))
                .bearer_auth(token)
                .json(&serde_json::json!({
                    "repositoryUrl": repository_url
                }))
                .send()
                .await?;

            let clone_json: serde_json::Value = clone_resp.json().await?;

            // Poll clone execution until "Success"
            let execution_url = clone_json["execution"]
                .as_str()
                .expect("missing execution url")
                .to_string();

            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project did not reach 'Ready' status after 20 attempts"
                    ));
                }

                attempts += 1;

                let resp = client.get(&execution_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = resp.json().await?;
                let status = json["status"].as_str().unwrap_or("");

                if status == "Success" {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait execution: attempts {}", attempts);
            }

            // Checkout branch (blocking)
            client
                .post(format!("{}/checkout", project_url))
                .bearer_auth(token)
                .json(&serde_json::json!({
                    "ref": "main"
                }))
                .send()
                .await?
                .error_for_status()?;

            // Trigger data retrival
            client
                .post(format!("{}/data", project_url))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?;

            // poll and wait for
            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project (file staging) did not reach 'Ready' status after 20 attempts"
                    ));
                }
                attempts += 1;

                let resp = client.get(&project_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = resp.json().await?;
                dbg!(&json);

                let is_all_slot_staged = slots.iter().all(|(key, _)| {
                    let slot = tool
                        .slots
                        .iter()
                        .find(|&s| s.name == *key)
                        // TODO: (jyu) This should be an error to tool developer (and who
                        // registered the tool), not to user. (But user can report to tool provider).
                        .expect("slot id should align between tool meta and input");
                    let s = json["dataStatus"][&slot.id]["status"]
                        .as_str()
                        .unwrap_or("");
                    s == "Available"
                });

                if is_all_slot_staged {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait datastaging: attempts {}", attempts);
            }

            // return the callback url

            let project_code = location
                .and_then(|loc| loc.split('/').next_back().map(|s| s.to_owned()))
                .expect("project id not there");

            tracing::info!("Project code: {}", project_code);

            let callback_url = Url::from_str(&format!("{}/projects/{}", backend_url, project_code))
                .expect("valid url");
            tracing::info!("Callback URL: {}", callback_url);

            // XXX: get status should be moved to monitor_state.
            // the state monitor is a dummy one that directly send Ready signal.
            // It should be send a stream with updating states.

            ///// -----------
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else {
            panic!("unknown support VRE");
        }
    }

    async fn get_artifact(&self, handler_id: &Uuid) -> anyhow::Result<Artifact> {
        let db = self.db.read().await;
        let hd = db.get(handler_id).unwrap();
        let artifact = hd.artifact.clone();
        Ok(artifact)
    }

    async fn query_tasks(&self, uid: &str) -> anyhow::Result<Vec<TaskHandler>> {
        let db = self.db.read().await;
        let out = db
            .values()
            .filter(|th| th.user_id.0 == uid)
            .cloned()
            .collect::<Vec<_>>();
        Ok(out)
    }

    async fn get_state(&self, task_uuid: &Uuid) -> anyhow::Result<ToolState> {
        let db = self.db.read().await;
        let hd = db.get(task_uuid).unwrap();
        let status = hd.state.clone();
        Ok(status)
    }
}

async fn report_service_status(reporter: HealthReporter) {
    // TODO: the real report should report all sub-services by making health check to the source
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        reporter
            .set_serving::<DataplayerServiceServer<Dataplayer>>()
            .await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (health_report, health_service) = tonic_health::server::health_reporter();
    health_report
        .set_serving::<DatasetServiceServer<Dataplayer>>()
        .await;
    health_report
        .set_serving::<ToolServiceServer<ToolDatabase>>()
        .await;
    health_report
        .set_serving::<DataplayerServiceServer<Dataplayer>>()
        .await;
    tokio::spawn(report_service_status(health_report.clone()));

    tracing_subscriber::fmt()
        .with_env_filter("info") // filter logs by level
        .init();

    let addr = "0.0.0.0:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and they usually don't have many updates, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let db_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        std::env::var("POSTGRES_USER").unwrap_or("postgres".to_string()),
        std::env::var("POSTGRES_PASSWORD").unwrap_or("test".to_string()),
        std::env::var("POSTGRES_ADDRESS").unwrap_or("localhost".to_string()),
        std::env::var("POSTGRES_PORT").unwrap_or("5432".to_string()),
        std::env::var("FILE_DB").unwrap_or("filedb".to_string()),
    );
    let data_src = DatahuggerDataSource::new(&db_url).await?;
    let data_src = Arc::new(data_src);

    let data_relayer = DataRelayer::new(data_src);

    // fallback to the production deployment if not specified.
    let tool_registry_api = std::env::var("TOOL_REGISTRY_API")
        .unwrap_or("https://dev.tools-registry.eosc-data-commons.eu/api/v1".to_string());

    let root_api = Url::from_str(&tool_registry_api).expect("invalid url");
    let tool_src = Arc::new(ToolRegistry::new(root_api));
    let tool_src_cloned = Arc::clone(&tool_src);
    let tool_srv = ToolDatabase::new(tool_src_cloned);

    let dispatcher = Arc::new(MockDispatcher::new());
    let tool_src_cloned = Arc::clone(&tool_src);
    let data_player = Dataplayer::new(dispatcher, tool_src_cloned);

    Server::builder()
        .add_service(health_service)
        .add_service(DatasetServiceServer::new(data_relayer))
        .add_service(ToolServiceServer::new(tool_srv))
        .add_service(DataplayerServiceServer::new(data_player))
        .serve(addr)
        .await?;
    Ok(())
}
