pub mod req_packager_rpc {
    tonic::include_proto!("req_packager.v1");
}
use crate::req_packager_rpc::{
    assemble_service_server::{AssembleService, AssembleServiceServer},
    browse_dataset_response::{BrowsePhase, Event},
    browse_error::ErrorCode,
    dataset_service_server::{DatasetService, DatasetServiceServer},
    vre_entry::Vre,
    BrowseComplete, BrowseDatasetRequest, BrowseDatasetResponse, BrowseError, DatasetInfo,
    FileEntry, PackageAssembleRequest, PackageAssembleResponse, VreEntry, VreEoscInline, VreHosted,
    VreTyp,
};

use prost_types::Timestamp;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use url::Url;

use req_packager::VirtualResearchEnv;

fn current_timestamp() -> Timestamp {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    Timestamp {
        seconds: now.as_secs().cast_signed(),
        nanos: now.subsec_nanos().cast_signed(),
    }
}

#[async_trait::async_trait]
trait FilemetrixClient: Send + Sync + 'static {
    // get dataset information
    async fn get_dataset_info(&self, url_datarepo: &str, id: &str) -> anyhow::Result<DatasetInfo>;
    // list files in the dataset
    async fn list_files(&self, url_datarepo: &str, id: &str) -> anyhow::Result<Vec<FileEntry>>;
}

struct MockFilemetrixClient {}

impl MockFilemetrixClient {
    fn new() -> Self {
        MockFilemetrixClient {}
    }
}

#[async_trait::async_trait]
impl FilemetrixClient for MockFilemetrixClient {
    async fn get_dataset_info(&self, url_datarepo: &str, id: &str) -> anyhow::Result<DatasetInfo> {
        let dataset_info = DatasetInfo {
            // mock all fields, they are from filemetrix API call.
            url_datarepo: url_datarepo.to_string(),
            id_dataset: id.to_string(),
            description: "example01".to_string(),
            total_files: None,
            total_size_bytes: None,
            created_at: Some(Timestamp::default()),
            updated_at: Some(Timestamp::default()),
            tags: HashMap::new(),
        };
        Ok(dataset_info)
    }

    async fn list_files(&self, url_datarepo: &str, id: &str) -> anyhow::Result<Vec<FileEntry>> {
        todo!()
    }
}

pub struct DataRepoRelayer {
    // TODO: source of tool-registry, mocked by a JSON, in production can be just tool-registry
    // API call address.
    // TODO: source of type-registry, mocked by a JSON
    // TODO: source of data repositories, mocked by a sqlite, the arch here not clear, should this
    // all behind the filemetrix? Or get from filemetrix (seems better because I don't want RP
    // tangled directly with DB, it is good to have operations behind filemetrix and this is one of
    // the roles filemetrix need to play) the basic info and query from DB after?
    filemetrix: Arc<dyn FilemetrixClient>,
}

// XXX: the logic and transport mixed here, I need to have a DatasetBrowser for the inner browse
// logic, then I can do the same no matter for filemetrix, or self directy service, or mocked test.
#[allow(clippy::too_many_lines)]
#[tonic::async_trait]
impl DatasetService for DataRepoRelayer {
    type BrowseDatasetStream = ReceiverStream<Result<BrowseDatasetResponse, Status>>;

    /// browse dataset through filemetrix API calls.
    /// XXX: I am expecting more than what filemetrix can provide.
    /// I mock those functionalities here and request filemetrix to have thoes implemneted.
    /// I need a service to downlead files for quick assessing (like a caching, caching <100k files).
    async fn browse_dataset(
        &self,
        request: Request<BrowseDatasetRequest>,
    ) -> Result<Response<Self::BrowseDatasetStream>, Status> {
        println!("Got a request: {request:?}");
        let (tx, rx) = mpsc::channel(16);
        let filemetrix_client = Arc::clone(&self.filemetrix);

        tokio::spawn(async move {
            // INIT Phase
            let req = request.get_ref();
            let url_datarepo = &req.url_datarepo;
            let id = &req.id_dataset;

            let dataset_info = match filemetrix_client.get_dataset_info(url_datarepo, id).await {
                Ok(info) => info,
                Err(err) => {
                    let err = BrowseError {
                        code: ErrorCode::UnavailableFilemetrix as i32,
                        message: format!("unable to get dataset info of url: {url_datarepo} - id: {id}, because of filemetrix error: {err}"),
                        path: None,
                        fatal: true,
                    };
                    tx.send(Ok(BrowseDatasetResponse {
                        phase: BrowsePhase::PhaseInit as i32,
                        event: Some(Event::Error(err)),
                    }))
                    .await
                    .ok();

                    return;
                }
            };
            tx.send(Ok(BrowseDatasetResponse {
                phase: BrowsePhase::PhaseInit as i32,
                event: Some(Event::DatasetInfo(dataset_info.clone())),
            }))
            .await
            .ok();

            tx.send(Ok(BrowseDatasetResponse {
                phase: BrowsePhase::PhaseBrowsing as i32,
                event: Some(Event::Progress(req_packager_rpc::BrowseProgress {
                    files_scanned: 0,
                    bytes_scanned: 0,
                    percent: 0,
                    path: None,
                })),
            }))
            .await
            .ok();

            // Browsing, keep on sending file info of the dataset asynchronously
            let files = match filemetrix_client.list_files(url_datarepo, id).await {
                Ok(files) => files,
                Err(err) => {
                    let err = BrowseError {
                        code: ErrorCode::UnavailableFilemetrix as i32,
                        message: format!("unable to list files url: {url_datarepo} - id: {id}, because of filemetrix error: {err}"),
                        path: None,
                        fatal: true,
                    };
                    tx.send(Ok(BrowseDatasetResponse {
                        phase: BrowsePhase::PhaseInit as i32,
                        event: Some(Event::Error(err)),
                    }))
                    .await
                    .ok();

                    return;
                }
            };

            let mut files_count = 0;
            let mut bytes_count = 0;
            // TODO: I may want to have pagination to at most showing 100 entries by default.
            // I need then have sever wait for incomming message to continue, bilateral required
            // and input needs to be a stream.
            for file in files {
                let filepath = file.path.clone();
                let sizebytes = file.size_bytes;
                if let Err(err) = tx
                    .send(Ok(BrowseDatasetResponse {
                        phase: BrowsePhase::PhaseBrowsing as i32,
                        event: Some(Event::FileEntry(file)),
                    }))
                    .await
                {
                    // Err
                    let err = BrowseError {
                        code: ErrorCode::UnavailableFile as i32,
                        message: format!("unable to send file: {url_datarepo} - id: {id} - file: {filepath} to client, because of: {err}"),
                        path: None,
                        fatal: true,
                    };
                    tx.send(Ok(BrowseDatasetResponse {
                        phase: BrowsePhase::PhaseInit as i32,
                        event: Some(Event::Error(err)),
                    }))
                    .await
                    .ok();
                } else {
                    // Ok
                    files_count += 1;
                    bytes_count += sizebytes;
                    tx.send(Ok(BrowseDatasetResponse {
                        phase: BrowsePhase::PhaseBrowsing as i32,
                        event: Some(Event::Progress(req_packager_rpc::BrowseProgress {
                            files_scanned: files_count,
                            bytes_scanned: bytes_count,
                            #[allow(clippy::cast_possible_truncation)]
                            percent: (files_count / dataset_info.total_files() * 100) as u32,
                            path: None,
                        })),
                    }))
                    .await
                    .ok();
                };

                // TODO: further operations include:
                // 1. file download, provide here? yes and calling scanning for mime-type and
                //    checksum automatically if the file is small (this rely on the file size must
                //    know beforehead).
                // 3. mime type deduct?? should this purely be the responsibility of filemetrix??
                //    (yes here)
                // 2. relay file to the VREs? in a separated step? (in the seprated step)
            }

            let success = files_count == dataset_info.total_files()
                && bytes_count == dataset_info.total_size_bytes();

            tx.send(Ok(BrowseDatasetResponse {
                phase: BrowsePhase::PhaseCompleted as i32,
                event: Some(Event::Complete(BrowseComplete {
                    total_files: files_count,
                    total_size_bytes: bytes_count,
                    success,
                    finish_at: Some(current_timestamp()),
                })),
            }))
            .await
            .ok();
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[async_trait::async_trait]
trait ToolRegistryClient: Send + Sync + 'static {
    // get tool info by id
    async fn get_tool(&self, id: &str) -> anyhow::Result<VirtualResearchEnv>;
    // list tools in the registry, fine to return a Vec store in the ram can handle 10,000 entries.
    async fn list_tools(&self) -> anyhow::Result<Vec<VirtualResearchEnv>>;
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

// this is supposed to be the ro-crate that contain all information to launch the vre with required
// data pointers, so dispatcher or vre (depends on design of the dispatcher) can access the data
// without the needs to store data in the middleware.
struct LaunchReq {
    // blob: Type
    id_vre: String,
    files: Vec<FileEntry>,
}

struct InfoRequest {}

#[async_trait::async_trait]
trait DispatcherClient: Send + Sync + 'static {
    // list all vre requests and their status
    async fn check_user_requests(&self, id_user: String) -> anyhow::Result<Vec<InfoRequest>>;
    // launch a vre with the launch request, return the callback url when it is ready
    async fn launch(&self, p: LaunchReq) -> anyhow::Result<Url>;
}

struct MockDispatcherClient {}

#[async_trait::async_trait]
impl DispatcherClient for MockDispatcherClient {
    async fn check_user_requests(&self, id_user: String) -> anyhow::Result<Vec<InfoRequest>> {
        todo!()
    }

    // launch a vre with the launch request, return the callback url when it is ready
    async fn launch(&self, p: LaunchReq) -> anyhow::Result<Url> {
        todo!()
    }
}

pub struct ReqPackAssembler {
    tool_registry: Arc<dyn ToolRegistryClient>,
}

#[tonic::async_trait]
impl AssembleService for ReqPackAssembler {
    async fn package_assemble(
        &self,
        request: Request<PackageAssembleRequest>,
    ) -> Result<Response<PackageAssembleResponse>, Status> {
        println!("Got a request: {request:?}");
        let tool_registry = Arc::clone(&self.tool_registry);

        // tool from tool registry and validate
        let req = request.get_ref();
        let id_vre = &req.id_vre;
        let files = &req.file_entries;

        let tool = tool_registry.get_tool(id_vre).await.map_err(|e| {
            // convert anyhow error to tonic status
            println!("Failed to get tool from registry: {e:?}");
            Status::internal(format!("Failed to get tool from registry: {e}"))
        })?;

        // TODO: assemble an ro-crate and send to dispatcher and get back the required vre callback
        match tool {
            VirtualResearchEnv::EoscInline { .. } => {
                // check file number and simply relay (because I use same data structure for the
                // tool registry api call) the entry to the client

                // Inline tool only support passing one file, there might be use cases the tool
                // processes multiple files, but impl that when the case comes.
                if files.len() != 1 {
                    let err_msg = format!(
                        "inline tool only processes on one file, get: {}",
                        files.len()
                    );
                    // TODO: proper tracing log
                    println!("{err_msg}");
                    return Err(Status::internal(err_msg));
                }

                // vre that not through dispatcher.
                let resp = PackageAssembleResponse {
                    vre_entry: Some(vre_entry),
                };
                Ok(Response::new(resp))
            }
            VirtualResearchEnv::Hosted { .. } => {
                // assamble a package and send to dispatcher that return a callback url

                // TODO: can check if the quota reached, users should not allowed to launch
                // infinit amount of vres (avoiding ddos).

                todo!()
            }
            _ => unimplemented!(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and based on their are less updated, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let filemetrix = Arc::new(MockFilemetrixClient::new());
    let relayer = DataRepoRelayer { filemetrix };

    let tool_registry = Arc::new(MockToolRegistryClient::new());
    let assembler = ReqPackAssembler { tool_registry };

    Server::builder()
        .add_service(DatasetServiceServer::new(relayer))
        .add_service(AssembleServiceServer::new(assembler))
        .serve(addr)
        .await?;
    Ok(())
}
