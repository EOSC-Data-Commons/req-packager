use async_stream::stream;
use futures_core::stream::BoxStream;
use prost_types::Timestamp;
use rand::{rng, seq::IndexedRandom, RngExt};
use req_packager::{
    grpc::{
        self, assemble_service_server::AssembleServiceServer,
        dataset_service_server::DatasetServiceServer,
    },
    DataRepoRelayer, DispatcherClient, FilemetrixClient, InfoRequest, LaunchRequset,
    ReqPackAssembler, ToolRegistryClient,
};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::{collections::HashMap, sync::Arc};
use tonic::transport::Server;
use url::Url;

use req_packager::VirtualResearchEnv;

#[derive(Clone)]
struct Dataset {
    // XXX: I don't want to couple the grpc logic with business logic, so I need real type for both
    // datasetinfo and fileentry.
    info: DatasetInfo,
    files: Vec<FileEntry>,
}

struct MockFilemetrixClient {
    // the key is a tuple, where 1st element is for datarepo url and the second is the id of the
    // dataset in the datarepo.
    datasets: HashMap<(String, String), Dataset>,
}

impl MockFilemetrixClient {
    fn new(datasets: Vec<Dataset>) -> Self {
        let datasets: HashMap<(String, String), Dataset> = datasets
            .into_iter()
            .map(|ds| {
                let info = ds.info.clone();
                let (url, id_ds) = (info.url, info.id);
                ((url, id_ds), ds)
            })
            .collect();
        MockFilemetrixClient { datasets }
    }
}

#[async_trait::async_trait]
impl FilemetrixClient for MockFilemetrixClient {
    async fn get_dataset_info(
        &self,
        url_datarepo: &str,
        id: &str,
    ) -> anyhow::Result<grpc::DatasetInfo> {
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

struct MockToolRegistryClient {}

impl MockToolRegistryClient {
    fn new() -> Self {
        MockToolRegistryClient {}
    }
}

#[async_trait::async_trait]
impl ToolRegistryClient for MockToolRegistryClient {
    async fn get_tool(&self, id: &str) -> anyhow::Result<VirtualResearchEnv> {
        todo!()
    }
    async fn list_tools(&self) -> anyhow::Result<Vec<VirtualResearchEnv>> {
        todo!()
    }
}

struct MockDispatcherClient {
    // I assume dispatcher knows and communicate with tool registry as well
    // It can be generic out to the `ToolRegistryClient` trait
    tool_registry: MockToolRegistryClient,
}

impl MockDispatcherClient {
    fn new() -> Self {
        MockDispatcherClient {
            tool_registry: MockToolRegistryClient::new(),
        }
    }
}

#[async_trait::async_trait]
impl DispatcherClient for MockDispatcherClient {
    async fn check_user_requests(&self, id_user: String) -> anyhow::Result<Vec<InfoRequest>> {
        todo!()
    }

    // launch a vre with the launch request, return the callback url when it is ready
    async fn launch(&self, p: LaunchRequset) -> anyhow::Result<Url> {
        // TODO: in the production impl, the launchReq -> ro-crate that carry information to launch
        // a vre.
        // It will be things like
        //
        // ```rust
        // struct RoCrate {
        //
        // }
        // let launch_pack: RoCrate = p.into();
        // let url = self.post(launch_pack).await?;
        // return url;
        // ```

        // TODO: dispatcher talk to tool registry to validate the tool request, this comes with the
        // question, should dispatcher fully trust req-packager that it always give the correct
        // tool id and type to launch. After all it is dispatcher's side decision whether do the
        // validation.
        // XXX: the LaunchRequset should contain the id of tool registry as well because dispatcher
        // in principle can support dispatch to different tool registry, but now only one is
        // enough.
        //
        // it also relates to the auth problem, who has the access to the vre? who should control
        // the permission of vre. I think it should be the vre provider and somewhere there is a
        // mapping for what eosc user can access which vres. Should this all kept in an auth server
        // (assume it will be one), or dispatcher maintain the table and mapping??

        todo!()
    }
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
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
        let total_size_bytes = rng.random_range(10_000_000..500_000_000);

        let mut tags = HashMap::new();
        for (k, v) in sample_tags.sample(&mut rng, 3) {
            tags.insert(k.to_string(), v.to_string());
        }

        let info = DatasetInfo {
            url: format!("https://example.com/datasets/{i}"),
            id: Uuid::new_v4().to_string(),
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and based on their are less updated, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let datasets = generate_datasets();
    let filemetrix = Arc::new(MockFilemetrixClient::new(datasets));
    let relayer = DataRepoRelayer::new(filemetrix);

    let tool_registry = Arc::new(MockToolRegistryClient::new());
    let dispacher = Arc::new(MockDispatcherClient::new());
    let assembler = ReqPackAssembler {
        tool_registry,
        dispacher,
    };

    Server::builder()
        .add_service(DatasetServiceServer::new(relayer))
        .add_service(AssembleServiceServer::new(assembler))
        .serve(addr)
        .await?;
    Ok(())
}
