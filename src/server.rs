use async_stream::stream;
use futures_core::stream::BoxStream;
use prost_types::Timestamp;
use rand::{rng, seq::IndexedRandom, RngExt};
use req_packager::{
    Artifact, DataRelayer, DataSource, Dataplayer, DatasetInfo, Dispatcher, DispatcherClient, FileEntry, HandlerId, InfoRequest, LaunchRequset, TaskHandler, ToolDatabase, ToolMeta, ToolRegistryClient, ToolSource, ToolState, UserId, Value, grpc::{
        self, ToolTaskHandler, dataplayer_service_server::DataplayerServiceServer, dataset_service_server::DatasetServiceServer, tool_service_server::{ToolService, ToolServiceServer}, tool_state
    }
};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use std::{collections::HashMap, str::FromStr, sync::Arc};
use tonic::transport::Server;
use url::Url;

use req_packager::VirtualResearchEnv;

#[derive(Clone, Debug)]
struct Dataset {
    // XXX: I don't want to couple the grpc logic with business logic, so I need real type for both
    // datasetinfo and fileentry.
    info: DatasetInfo,
    files: Vec<FileEntry>,
}

struct MockDataSource {
    // the key is a tuple, where 1st element is for datarepo url and the second is the id of the
    // dataset in the datarepo.
    datasets: HashMap<String, Dataset>,
}

impl MockDataSource {
    fn new(datasets: Vec<Dataset>) -> Self {
        let datasets: HashMap<String, Dataset> = datasets
            .into_iter()
            .map(|ds| {
                let info = ds.info.clone();
                let uuid = compute_uuid_from_string(&format!("{}/{}", info.url, info.id));
                (uuid.to_string(), ds)
            })
            .collect();
        MockDataSource { datasets }
    }
}

#[async_trait::async_trait]
impl DataSource for MockDataSource {
    async fn get_dataset_info(&self, uuid: &str) -> anyhow::Result<DatasetInfo> {
        // XXX: very fragile to use url+id, should be a PID or other primary key in DB.
        match self.datasets.get(uuid) {
            Some(dataset) => {
                let info = dataset.info.clone();
                Ok(info.into())
            }
            _ => {
                anyhow::bail!("didn't find the dataset with {:?}", uuid)
            }
        }
    }

    async fn list_files(&self, uuid: &str) -> anyhow::Result<BoxStream<'static, FileEntry>> {
        match self.datasets.get(uuid) {
            Some(dataset) => {
                let files = dataset
                    .files
                    .iter()
                    .map(|f| f.clone().into())
                    .collect::<Vec<grpc::FileEntry>>();
                let stream = Box::pin(stream! {
                    for file in files {
                        yield file.into();
                    }
                });
                Ok(stream)
            }
            _ => {
                anyhow::bail!("didn't find the dataset with {:?}", uuid)
            }
        }
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
    async fn search_tools_by_text(&self, text: &str) -> anyhow::Result<Vec<ToolMeta>> {
        todo!()
    }

    async fn find_tools(&self, files: &[FileEntry]) -> anyhow::Result<Vec<ToolMeta>> {
        // XXX: very dummy to guess tool by number of files, it needs to be the file mime-type,
        // even in PoC. smart a bit on n % 10.
        let tools = match files.len() {
            1 => self.tools[0..1].to_vec(),
            2 => self.tools[0..2].to_vec(),
            3 => self.tools[0..3].to_vec(),
            _ => self.tools[0..4].to_vec(),
        };
        Ok(tools)
    }
    async fn get_tool(&self, id: &str) -> anyhow::Result<ToolMeta> {
        Ok(self.tools[0].clone())
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

        let id = uuid::Uuid::new_v4();
        let artifact = Artifact::EoscInlineTool {
            callback: Url::from_str("https://example.com/launch").unwrap(),
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
        Ok(status.clone())
    }
}

fn generate_fake_files(total: u64) -> Vec<FileEntry> {
    let mut rng = rng();
    let now = Utc::now();

    let mime_types = [
        "text/csv",
        "application/json",
        "application/parquet",
        "image/png",
        "application/octet-stream",
    ];

    let mut entries = Vec::new();

    // Create some directory structure first
    let dirs = vec!["raw", "processed", "results", "metadata"];

    for dir in &dirs {
        entries.push(FileEntry {
            download_url: None,
            path: dir.to_string(),
            is_dir: true,
            size_bytes: 0,
            mime_type: None,
            checksum: None,
            modified_at: now - Duration::days(rng.random_range(1..30)),
        });
    }

    // XXX: this is very dummy, the file itself not conform with the mimetype.
    let download_urls = [
        "https://filesamples.com/samples/image/hdr/sample_640%C3%97426.hdr",
        "https://filesamples.com/samples/image/png/sample_640%C3%97426.png",
        "https://filesamples.com/samples/image/png/sample_5184%C3%973456.png",
        "https://filesamples.com/samples/image/tiff/sample_1280%C3%97853.tiff",
    ];

    // Generate files inside directories
    for i in 0..total {
        let parent = dirs.choose(&mut rng).unwrap();

        let size = rng.random_range(10_000..10_000_000);
        let modified = now - Duration::days(rng.random_range(0..30));

        let mime = mime_types.choose(&mut rng).unwrap();
        let path = format!("{parent}/file_{i}.dat");
        let download_url = download_urls.choose(&mut rng).unwrap();

        entries.push(FileEntry {
            download_url: Some(download_url.to_string()),
            // XXX: this may not be used in the ui in the end, but it should be the
            // __ROOT__<path>
            path,
            is_dir: false,
            size_bytes: size,
            mime_type: Some(mime.to_string()),
            checksum: Some(Uuid::new_v4().to_string()),
            modified_at: modified,
        });
    }

    entries
}

fn compute_uuid_from_string(input: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, input.as_bytes())
}

fn generate_datasets() -> Vec<Dataset> {
    let mut rng = rng();

    let mut datasets = Vec::new();

    let sample_tags = [
        ("domain", "physics"),
        ("type", "simulation"),
        ("format", "csv"),
        ("owner", "research-team"),
        ("status", "validated"),
    ];

    for i in 0..5 {
        let now = Utc::now();
        let created = now - Duration::days(rng.random_range(10..100));
        let updated = created + Duration::days(rng.random_range(1..10));

        let total_files = rng.random_range(1..10);
        let total_size_bytes = rng.random_range(10_000_000..500_000_000);

        let mut tags = HashMap::new();
        for (k, v) in sample_tags.sample(&mut rng, 3) {
            tags.insert(k.to_string(), v.to_string());
        }

        let input = format!("https://example.com/datasets/{i}");
        let uuid = compute_uuid_from_string(&input);
        let info = DatasetInfo {
            uuid,
            url: "https://example.com/datasets".to_string(),
            id: format!("{i}"),
            description: format!("Mock dataset number {i}"),
            total_files: Some(total_files),
            total_size_bytes: Some(total_size_bytes),
            created_at: Some(created),
            updated_at: Some(updated),
            tags,
        };

        let files = generate_fake_files(total_files);

        datasets.push(Dataset { info, files });
    }

    datasets
}

fn generate_tools() -> Vec<ToolMeta> {
    (0..4)
        .map(|i| ToolMeta {
            id: format!("{i}"),
            version: "0.1.0alpha".to_string(),
            name: "".to_uppercase(),
            uri: "".to_uppercase(),
            types: vec![],
            description: "".to_uppercase(),
            slots: vec![],
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and based on their are less updated, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let datasets = generate_datasets();
    let data_src = Arc::new(MockDataSource::new(datasets));
    let data_relayer = DataRelayer::new(data_src);

    let tools = generate_tools();
    let tool_src = Arc::new(MockToolSrc::new(tools));
    let tool_src_cloned = Arc::clone(&tool_src);
    let tool_srv = ToolDatabase::new(tool_src_cloned);

    let dispatcher = Arc::new(MockDispatcher::new());
    let tool_src_cloned = Arc::clone(&tool_src);
    let data_player = Dataplayer::new(dispatcher, tool_src_cloned);

    Server::builder()
        .add_service(DatasetServiceServer::new(data_relayer))
        .add_service(ToolServiceServer::new(tool_srv))
        .add_service(DataplayerServiceServer::new(data_player))
        .serve(addr)
        .await?;
    Ok(())
}
