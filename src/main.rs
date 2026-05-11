use datahugger::{
    crawl,
    crawler::{CrawlerError, ProgressManager},
    resolve, resolve_doi_to_url, Entry,
};
use exn::Exn;
use futures_core::stream::BoxStream;
use futures_util::StreamExt;
use hyper::header::CONTENT_TYPE;
use indicatif::ProgressBar;
use req_packager::{
    grpc::{
        dataplayer_service_server::DataplayerServiceServer,
        dataset_service_server::DatasetServiceServer, tool_service_server::ToolServiceServer,
    },
    Artifact, DataRelayer, DataSource, Dataplayer, DatasetInfo, Dispatcher, FileEntry, HandlerId,
    Slot, TaskHandler, ToolDatabase, ToolMeta, ToolSource, ToolState, UserId, Value,
};
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT},
    Client, ClientBuilder,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tonic_health::server::HealthReporter;

use tokio::sync::RwLock;
use uuid::Uuid;

use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, LazyLock},
};
use tonic::transport::Server;
use url::Url;

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
        crawl(
            client.clone(),
            Arc::clone(&self.backend),
            root_dir,
            mp.clone(),
        )
        .filter_map(|res| async move {
            match res {
                // TODO: need dir as well for the layout in the UI.
                Ok(Entry::Dir(_)) => None,
                Ok(Entry::File(f)) => {
                    let f: FileEntry = f.into();
                    Some(Ok(f))
                }
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

struct DatahuggerDataSource;

impl DatahuggerDataSource {
    fn new() -> Self {
        DatahuggerDataSource
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
        let ds = resolve(&url).await.map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let mp = NoProgress;
        let files = ds
            .crawl_file(&client, mp)
            // TODO: I need log on error cases on the server.
            .filter_map(|f| async move { f.ok() })
            .boxed();
        Ok(files)
    }
}

struct FileMetrixAsDataSource {
    pool_dataset: PgPool,
    pool_filedb: PgPool,
}

impl FileMetrixAsDataSource {
    fn new(pool_dataset: PgPool, pool_filedb: PgPool) -> Self {
        FileMetrixAsDataSource {
            pool_dataset,
            pool_filedb,
        }
    }
}

#[async_trait::async_trait]
impl DataSource for FileMetrixAsDataSource {
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

        // let info: (i64,) = sqlx::query_as("SELECT $1")
        //     .bind(150_i64)
        //     .fetch_one(&self.pool_dataset)
        //     .await?;
        Ok(info)
    }

    async fn list_files(&self, uuid: &str) -> anyhow::Result<BoxStream<'static, FileEntry>> {
        // let user_agent = format!(
        //     "datahugger-over-eosc-coordinator/{}",
        //     env!("CARGO_PKG_VERSION")
        // );
        // let mut headers = HeaderMap::new();
        // if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        //     headers.insert(
        //         AUTHORIZATION,
        //         HeaderValue::from_str(&format!("token {token}"))?,
        //     );
        // }
        // if let Ok(token) = std::env::var("DRYAD_API_TOKEN") {
        //     headers.insert(
        //         AUTHORIZATION,
        //         HeaderValue::from_str(&format!("Bearer {token}"))?,
        //     );
        // }
        // headers.insert(USER_AGENT, HeaderValue::from_str(&user_agent)?);
        // let client = ClientBuilder::new()
        //     .user_agent(user_agent)
        //     .default_headers(headers)
        //     .use_native_tls()
        //     .build()?;
        // let mut url = uuid.to_string();
        // if url.starts_with("https://doi.org/") {
        //     let doi = url.trim_start_matches("https://doi.org/");
        //     url = resolve_doi_to_url(&client, doi, true)
        //         .await
        //         .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        // }
        // let ds = resolve(&url).await.map_err(|e| anyhow::anyhow!("{e:?}"))?;
        // let mp = NoProgress;
        // let files = ds
        //     .crawl_file(&client, mp)
        //     // TODO: I need log on error cases on the server.
        //     .filter_map(|f| async move { f.ok() })
        //     .boxed();
        // Ok(files)
        //
        // let info: (i64,) = sqlx::query_as("SELECT $1")
        //     .bind(150_i64)
        //     .fetch_one(&self.pool_filedb)
        //     .await?;
        let files = vec![];
        let files = futures::stream::iter(files);

        Ok(Box::pin(files))
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
    // TODO: file_formats: Vec<String>,
}

impl From<ResponseSlot> for Slot {
    fn from(value: ResponseSlot) -> Self {
        Slot {
            id: value.id,
            slot_type: value.slot_type,
            name: value.name,
        }
    }
}

#[derive(Deserialize, Debug)]
struct OneToolResponse {
    // XXX: @reggie, maybe this better to be some special string type id?
    id: u64,
    uri: String,
    name: String,
    description: String,
    types: Vec<String>,
    version: String,
    input_slots: Vec<ResponseSlot>,
}

static TOOLS: LazyLock<Vec<ToolMeta>> = LazyLock::new(|| {
    vec![
        ToolMeta {
            id: "::st:001".to_string(),
            version: "v0".to_string(),
            name: "mybinder".to_string(),
            uri: "https://mybinder.org/".to_string(),
            types: vec!["general_tool".to_string(), "mybinder".to_string()],
            description: "mybinder as genenal tool".to_string(),
            slots: vec![],
        },
        ToolMeta {
            id: "::st:002".to_string(),
            version: "v0".to_string(),
            name: "Reproduciple Research Platform (RRP)".to_string(),
            uri: "https://rrp-eosc.ethz.ch/".to_string(),
            types: vec!["general_tool".to_string(), "rrp".to_string()],
            description: "RRP as genenal tool".to_string(),
            slots: vec![Slot {
                id: "image_name".to_string(),
                name: "Image Name".to_string(),
                slot_type: "string".to_string(),
            }],
        },
        ToolMeta {
            id: "::st:003".to_string(),
            version: "v0".to_string(),
            name: "CernBox".to_string(),
            uri: "cernbox.cern.ch".to_string(),
            types: vec!["general_tool".to_string(), "cernbox".to_string()],
            description: "Tool to send files to CernBox user".to_string(),
            slots: vec![],
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
        let resp: Vec<OneToolResponse> = resp.json().await?;
        // tracing::info!("resp is: {:?}", resp);
        let tools = resp
            .into_iter()
            .map(|res| {
                let slots = res
                    .input_slots
                    .into_iter()
                    .map(|s| s.into())
                    .collect::<Vec<_>>();

                ToolMeta {
                    id: res.id.to_string(),
                    version: res.version,
                    uri: res.uri,
                    types: res.types,
                    name: res.name,
                    description: res.description,
                    slots,
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

        let response: Vec<OneToolResponse> = response.json().await.inspect_err(|err| {
            // dbg!(err);
        })?;
        let tools = response
            .into_iter()
            .map(|res| {
                let slots = res
                    .input_slots
                    .into_iter()
                    .map(|s| s.into())
                    .collect::<Vec<_>>();

                ToolMeta {
                    id: res.id.to_string(),
                    version: res.version,
                    uri: res.uri,
                    types: res.types,
                    name: res.name,
                    description: res.description,
                    slots,
                }
            })
            .collect::<Vec<_>>();
        return Ok(tools);
    }

    async fn get_tool(&self, id: &str) -> anyhow::Result<ToolMeta> {
        if id.starts_with("::st") {
            if let Some(tool) = TOOLS.to_vec().iter().find(|&t| t.id == id) {
                return Ok(tool.to_owned());
            }
        }
        let url = format!("{}/tools/{}", self.root_api.as_str(), id);
        let resp: OneToolResponse = reqwest::get(url).await?.json().await?;
        let slots = resp
            .input_slots
            .into_iter()
            .map(|s| s.into())
            .collect::<Vec<_>>();

        let tool = ToolMeta {
            id: id.to_string(),
            version: resp.version,
            uri: resp.uri,
            types: resp.types,
            name: resp.name,
            description: resp.description,
            slots,
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
        uid: &str,
        tool: &ToolMeta,
        dataset: &str,
        parameters: &HashMap<String, Value>,
        files: &HashMap<String, FileEntry>,
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

            let request_state: serde_json::Map<String, serde_json::Value> = files
                .iter()
                .filter_map(|(key, entry)| {
                    let location = entry.download_url.as_deref()?;

                    let filetype = entry.path.rsplit('.').next().unwrap_or("txt");

                    Some((
                        key.clone(),
                        // @reggie galaxy specific info
                        serde_json::json!({
                            "class": "File",
                            "filetype": filetype,
                            "location": location
                        }),
                    ))
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
            if let Ok(key) = std::env::var("VIP_API_KEY") {
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

            // TODO:
            let request_state: serde_json::Map<String, serde_json::Value> = files
                .iter()
                .filter_map(|(key, entry)| {
                    // VIP use the slot id as the key of the file list in the payload
                    let location = entry.download_url.as_deref()?;

                    let slot_id = tool
                        .slots
                        .iter()
                        .find(|s| s.name == *key)
                        .map(|s| s.id.clone())?;

                    Some((slot_id, serde_json::Value::String(location.to_string())))
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
            // TODO: need to use the helper function I provide in datahugger to get the branch or
            // commit number.
            let callback_url = Url::from_str("https://mybinder.org/v2/gh/binder-examples/r/main")
                .expect("a valid url");

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
            todo!()
        } else if tool.types.contains(&"rrp".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            let backend_url = "https://rrp-eosc.ethz.ch";

            let client = Client::builder().build()?;
            let Some(image_name) = parameters.get("Image Name").map(|v| {
                let serde_json::Value::String(v) = v.get_inner() else {
                    unreachable!("must be a string")
                };
                v
            }) else {
                unreachable!("'Image Name' must be set")
            };

            // ---- CREATE PROJECT ----
            let project_data = serde_json::json!({
                "type": "createFromExternalCatalog",
                "image": image_name,
                // "image": "reproducibleresearchplatform/rrp-tst:q75v54b-cunya",
                "environmentType": "jupyterlab",
            });

            let Ok(oidc_agent_token) = std::env::var("OIDC_AGENT_TOKEN") else {
                panic!("oidc_agent_token not found in env var")
            };
            let resp = client
                .post(format!("{}/api/projects", backend_url))
                .bearer_auth(oidc_agent_token)
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
            dbg!("go here");

            tracing::info!("Create project: {}", resp.status());

            let location = resp.headers().get("Location").and_then(|v| v.to_str().ok());

            let project_code = location
                .and_then(|loc| loc.split('/').next_back())
                .expect("project id not there");

            tracing::info!("Project code: {}", project_code);

            let callback_url = Url::from_str(&format!("{}/projects/{}", backend_url, project_code))
                .expect("valid url");
            tracing::info!("Callback URL: {}", callback_url);

            // XXX: get status should be moved to monitor_state.
            // the state monitor is a dummy one that directly send Ready signal.
            // It should be send a stream with updating states.
            //
            // // ---- GET STATUS ----
            // let resp = client
            //     .get(format!("{}/api/projects/{}", backend_url, project_code))
            //     .send()
            //     .await?;
            //
            // tracing::info!("Status: {}", resp.status());
            // let json: serde_json::Value = resp.json().await?;
            //
            // tracing::debug!("Body: {}", json);

            // // ---- START PROJECT ----
            // let start_req = serde_json::json!({
            //     "type": "start",
            //     "remote": false,
            // });
            //
            // let resp = client
            //     .post(format!("{}/api/projects/{}", backend_url, project_code))
            //     .headers(headers)
            //     .json(&start_req)
            //     .send()
            //     .await?;
            //
            // println!("Start: {}", resp.status());

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

    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and they usually don't have many updates, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let data_src = Arc::new(DatahuggerDataSource::new());
    let data_relayer = DataRelayer::new(data_src);

    let root_api = Url::from_str("http://tool-registry.eosc-data-commons.dansdemo.nl/api/v1")
        .expect("invalid url");
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
