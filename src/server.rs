use async_stream::stream;
use futures_core::stream::BoxStream;
use prost_types::Timestamp;
use rand::{rng, seq::IndexedRandom, RngExt};
use req_packager::{
    grpc::{
        self,
        dataplayer_service_server::DataplayerServiceServer,
        dataset_service_server::DatasetServiceServer,
        tool_service_server::{ToolService, ToolServiceServer},
        tool_status, ToolHandler, ToolMeta, UserId,
    },
    Artifact, DataRelayer, DataSource, Dataplayer, Dispatcher, DispatcherClient, InfoRequest,
    LaunchRequset, ToolDatabase, ToolRegistryClient, ToolSource, ToolStatus,
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
    datasets: HashMap<(String, String), Dataset>,
}

impl MockDataSource {
    fn new(datasets: Vec<Dataset>) -> Self {
        let datasets: HashMap<(String, String), Dataset> = datasets
            .into_iter()
            .map(|ds| {
                let info = ds.info.clone();
                let (url, id_ds) = (info.url, info.id);
                ((url, id_ds), ds)
            })
            .collect();
        MockDataSource { datasets }
    }
}

#[async_trait::async_trait]
impl DataSource for MockDataSource {
    async fn get_dataset_info(
        &self,
        url_datarepo: &str,
        id: &str,
    ) -> anyhow::Result<grpc::DatasetInfo> {
        // XXX: very fragile to use url+id, should be a PID or other primary key in DB.
        match self
            .datasets
            .get(&(url_datarepo.to_string(), id.to_string()))
        {
            Some(dataset) => {
                let info = dataset.info.clone();
                Ok(info.into())
            }
            _ => {
                anyhow::bail!("didn't find the dataset with {:?}", (url_datarepo, id))
            }
        }
    }

    fn list_files(
        &self,
        url_datarepo: &str,
        id: &str,
    ) -> anyhow::Result<BoxStream<'static, grpc::FileEntry>> {
        match self
            .datasets
            .get(&(url_datarepo.to_string(), id.to_string()))
        {
            Some(dataset) => {
                let files = dataset
                    .files
                    .iter()
                    .map(|f| f.clone().into())
                    .collect::<Vec<grpc::FileEntry>>();
                let stream = Box::pin(stream! {
                    for file in files {
                        yield file;
                    }
                });
                Ok(stream)
            }
            _ => {
                anyhow::bail!("didn't find the dataset with {:?}", (url_datarepo, id))
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
    async fn find_tools(&self, files: &[grpc::FileEntry]) -> anyhow::Result<Vec<ToolMeta>> {
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
}

struct MockDispatcher {
    // to get the handler and light meta data
    db_1: RwLock<HashMap<Uuid, ToolHandler>>,
    // to get the heavy Artifact, that is updated once per record
    db_2: RwLock<HashMap<Uuid, Artifact>>,
}

impl MockDispatcher {
    fn new() -> Self {
        Self {
            db_1: RwLock::new(HashMap::new()),
            db_2: RwLock::new(HashMap::new()),
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
        files: &[grpc::FileEntry],
    ) -> anyhow::Result<String> {
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
        let th = ToolHandler {
            state: Some(grpc::ToolStatus {
                log: "".to_string(),
                state: tool_status::State::Ready.into(),
            }),
            owner: Some(UserId {
                inner: uid.to_string(),
            }),
            id: id.to_string(),
        };

        let mut db = self.db_1.write().await;
        db.entry(id).or_insert(th.clone());
        // mock: this example is the lightweight tool that immediately ready, so the artifact is
        // ready immediately.

        let mut db = self.db_2.write().await;
        let art = Artifact::EoscInlineTool {
            callback: Url::from_str("https://example.com/launch").unwrap(),
        };
        db.entry(id).or_insert(art.clone());

        Ok(th.id)
    }

    async fn get_artifact(&self, handler_id: &str) -> anyhow::Result<Artifact> {
        let db = self.db_2.read().await;
        let hd = db.get(&Uuid::from_str(handler_id).unwrap()).unwrap();
        Ok(hd.clone())
    }

    async fn query_tools(&self, uid: &str) -> anyhow::Result<Vec<ToolHandler>> {
        let db = self.db_1.read().await;
        let out = db
            .iter()
            .filter_map(|(_uuid, th)| {
                let owner_id = th.clone().owner.unwrap();
                if uid == owner_id.inner {
                    Some(th.to_owned())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Ok(out)
    }

    async fn get_status(&self, handler_id: &str) -> anyhow::Result<ToolStatus> {
        let db = self.db_1.read().await;
        let hd = db.get(&Uuid::from_str(handler_id).unwrap()).unwrap();
        let status = hd.state.as_ref().unwrap();
        Ok(status.clone().into())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DatasetInfo {
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
    path: String,
    is_dir: bool,
    size_bytes: u64,
    mime_type: Option<String>,
    checksum: Option<String>,
    modified_at: DateTime<Utc>,
}

impl From<FileEntry> for grpc::FileEntry {
    fn from(f: FileEntry) -> Self {
        let modified_at = Timestamp {
            seconds: f.modified_at.timestamp(),
            nanos: 0,
        };
        grpc::FileEntry {
            path: f.path,
            is_dir: f.is_dir,
            size_bytes: f.size_bytes,
            mime_type: f.mime_type,
            checksum: f.checksum,
            modified_at: Some(modified_at),
        }
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
            path: dir.to_string(),
            is_dir: true,
            size_bytes: 0,
            mime_type: None,
            checksum: None,
            modified_at: now - Duration::days(rng.random_range(1..30)),
        });
    }

    // Generate files inside directories
    for i in 0..total {
        let parent = dirs.choose(&mut rng).unwrap();

        let size = rng.random_range(10_000..10_000_000);
        let modified = now - Duration::days(rng.random_range(0..30));

        let mime = mime_types.choose(&mut rng).unwrap();

        entries.push(FileEntry {
            path: format!("{parent}/file_{i}.dat"),
            is_dir: false,
            size_bytes: size,
            mime_type: Some(mime.to_string()),
            checksum: Some(Uuid::new_v4().to_string()),
            modified_at: modified,
        });
    }

    entries
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

        let total_files = rng.random_range(5..50);
        dbg!(total_files);
        let total_size_bytes = rng.random_range(10_000_000..500_000_000);

        let mut tags = HashMap::new();
        for (k, v) in sample_tags.sample(&mut rng, 3) {
            tags.insert(k.to_string(), v.to_string());
        }

        let info = DatasetInfo {
            url: "https://example.com/datasets".to_string(),
            id: format!("{i}"),
            description: format!("Mock dataset number {i}"),
            total_files: Some(total_files),
            total_size_bytes: Some(total_size_bytes),
            create_at: created,
            updated_at: updated,
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
