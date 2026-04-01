use datahugger::{
    crawl,
    crawler::{CrawlerError, ProgressManager},
    resolve, resolve_doi_to_url, Entry, FileMeta,
};
use exn::Exn;
use futures_core::stream::BoxStream;
use futures_util::StreamExt;
use indicatif::ProgressBar;
use prost_types::Timestamp;
use req_packager::{
    grpc::{
        self, dataplayer_service_server::DataplayerServiceServer,
        dataset_service_server::DatasetServiceServer, tool_service_server::ToolServiceServer,
    },
    Artifact, DataRelayer, DataSource, Dataplayer, Dispatcher, HandlerId, TaskHandler,
    ToolDatabase, ToolMeta, ToolSource, ToolState, UserId,
};
use tonic_health::server::HealthReporter;

use chrono::{DateTime, Utc};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use std::{collections::HashMap, str::FromStr, sync::Arc};
use tonic::transport::Server;
use url::Url;

struct DatahuggerDataSource;

impl DatahuggerDataSource {
    fn new() -> Self {
        DatahuggerDataSource
    }
}

trait CrawlFileExt {
    fn crawl_file(
        self,
        client: &Client,
        mp: impl ProgressManager,
    ) -> BoxStream<'static, Result<grpc::FileEntry, Exn<CrawlerError>>>;
}

impl CrawlFileExt for datahugger::Dataset {
    fn crawl_file(
        self,
        client: &Client,
        mp: impl ProgressManager,
    ) -> BoxStream<'static, Result<grpc::FileEntry, Exn<CrawlerError>>> {
        let root_dir = self.root_dir();
        crawl(
            client.clone(),
            Arc::clone(&self.backend),
            root_dir,
            mp.clone(),
        )
        .filter_map(|res| async move {
            match res {
                Ok(Entry::Dir(_)) => None,
                Ok(Entry::File(f)) => {
                    let f: FileEntry = f.into();
                    let f: grpc::FileEntry = f.into();
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

#[async_trait::async_trait]
impl DataSource for DatahuggerDataSource {
    async fn get_dataset_info(&self, uuid: &str) -> anyhow::Result<grpc::DatasetInfo> {
        let url = uuid;
        let info = grpc::DatasetInfo {
            url_datarepo: url.to_string(),
            id_dataset: "dummy".to_string(),
            description: "datahugger not yet support dataset metadata harvesting".to_string(),
            total_files: None,
            total_size_bytes: None,
            created_at: None,
            updated_at: None,
            tags: HashMap::new(),
        };
        Ok(info)
    }

    async fn list_files(&self, uuid: &str) -> anyhow::Result<BoxStream<'static, grpc::FileEntry>> {
        let user_agent = format!(
            "datahugger-over-eosc-coordinator/{}",
            env!("CARGO_PKG_VERSION")
        );
        let client = ClientBuilder::new().user_agent(user_agent).build()?;
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

struct MockToolSrc {
    tools: Vec<ToolMeta>,
}

impl MockToolSrc {
    fn new(tools: Vec<ToolMeta>) -> Self {
        MockToolSrc { tools }
    }
}

#[async_trait::async_trait]
impl ToolSource for MockToolSrc {
    async fn find_tools(&self, files: &[grpc::FileEntry]) -> anyhow::Result<Vec<ToolMeta>> {
        let tools = self.tools.clone();
        Ok(tools)
    }
    async fn get_tool(&self, id: &str) -> anyhow::Result<ToolMeta> {
        for tool in self.tools.iter() {
            if tool.id.as_str() == id {
                return Ok(tool.clone());
            }
        }

        anyhow::bail!("tool {id} not found")
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
        files: &HashMap<String, grpc::FileEntry>,
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
        // //   "id": "uuid-1",
        // //   "runtime": {
        // //     "config": {"workflow_id": "xxx", "workflow_target_type": "trs_url"}
        // //   }
        // // }
        //
        // proxy/plugin runs for every VRE in its own process and talk to dispatcher with a well
        // defined protocol.

        let workflow_id = match tool.id.as_str() {
            "uuid-1" => "https://dockstore.org/api/ga4gh/trs/v2/tools/%23workflow%2Fgithub.com%2Flaitanawe%2Fismb2024%2Fgalaxy_example/versions/main/PLAIN_GALAXY/descriptor//Galaxy-Workflow-reverse_file_galaxy_workflow.ga",
            "uuid-2" => "https://dockstore.org/api/ga4gh/trs/v2/tools/%23workflow%2Fgithub.com%2Fbwalkowi%2Fgalaxy-workflow-ocr-test%2Fmain/versions/main/PLAIN_GALAXY/descriptor//galaxy-workflow-ocr-test-DaSCH.ga",
            _ => panic!("this is a mock, crapy mock, but already tell much more than dispatcher."),
        };

        let request_state: serde_json::Map<String, serde_json::Value> = files
            .iter()
            .filter_map(|(key, entry)| {
                let location = entry.download_url.as_deref()?;

                let filetype = entry.path.rsplit('.').next().unwrap_or("txt");

                Some((
                    key.clone(),
                    // XXX: @reggie galaxy specific info
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

        let client = reqwest::Client::new();
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

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DatasetInfo {
    uuid: Uuid,
    url: String,
    id: String,
    description: String,
    total_files: Option<u64>,
    total_size_bytes: Option<u64>,
    create_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    tags: HashMap<String, String>,
}

impl From<DatasetInfo> for grpc::DatasetInfo {
    fn from(d: DatasetInfo) -> Self {
        let created_at = Timestamp {
            seconds: d.create_at.timestamp(),
            nanos: 0,
        };
        let updated_at = Timestamp {
            seconds: d.updated_at.timestamp(),
            nanos: 0,
        };
        grpc::DatasetInfo {
            url_datarepo: d.url,
            id_dataset: d.id,
            description: d.description,
            total_files: d.total_files,
            total_size_bytes: d.total_size_bytes,
            created_at: Some(created_at),
            updated_at: Some(updated_at),
            tags: d.tags,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FileEntry {
    download_url: Option<String>,
    path: String,
    is_dir: bool,
    size_bytes: u64,
    mime_type: Option<String>,
    checksum: Option<String>,
    modified_at: DateTime<Utc>,
}

impl From<FileMeta> for FileEntry {
    fn from(meta: FileMeta) -> Self {
        FileEntry {
            download_url: Some(meta.download_url().to_string()),
            path: meta.path().to_string(),
            // XXX: for some dataset, this can be a folder
            is_dir: false,
            // XXX: how to deal the case when size is unknown from datahugger?
            size_bytes: meta.size().unwrap_or(0),
            mime_type: meta.mimetype().map(|m| format!("{m}")),
            checksum: None,
            // XXX: modified time??
            modified_at: DateTime::from_timestamp_nanos(323),
        }
    }
}

impl From<FileEntry> for grpc::FileEntry {
    fn from(f: FileEntry) -> Self {
        let modified_at = Timestamp {
            seconds: f.modified_at.timestamp(),
            nanos: 0,
        };
        grpc::FileEntry {
            download_url: f.download_url,
            path: f.path,
            is_dir: f.is_dir,
            size_bytes: f.size_bytes,
            mime_type: f.mime_type,
            checksum: f.checksum,
            checksum_type: None, // TODO: ?
            modified_at: Some(modified_at),
        }
    }
}

fn generate_tools() -> Vec<ToolMeta> {
    let tool01 = ToolMeta {
        id: "uuid-1".to_string(),
        version: "v0".to_string(),
        name: "Text file reversion (Galaxy)".to_string(),
        description: "Reverse the content of a text file".to_string(),
        slots: vec!["simpletext_input".to_string()],
    };
    let tool02 = ToolMeta {
        id: "uuid-2".to_string(),
        version: "v0".to_string(),
        name: "OCR + word cloud (Galaxy)".to_string(),
        description: "Perform OCR on an image and generate a word cloud".to_string(),
        slots: vec!["Input Image".to_string(), "Upload Stopwords".to_string()],
    };
    vec![tool01, tool02]
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

    let tools = generate_tools();
    let tool_src = Arc::new(MockToolSrc::new(tools));
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
