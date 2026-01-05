pub mod req_packager {
    tonic::include_proto!("req_packager.v1");
}

use prost_types::Timestamp;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::req_packager::{
    browse_dataset_response::{BrowsePhase, Event},
    browse_error::ErrorCode,
    dataset_service_server::{DatasetService, DatasetServiceServer},
    BrowseComplete, BrowseDatasetRequest, BrowseDatasetResponse, BrowseError, DatasetInfo,
    FileEntry,
};
use tonic::{transport::Server, Request, Response, Status};

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
    async fn get_dataset_info(&self, repo_url: &str, id: &str) -> anyhow::Result<DatasetInfo>;
    // list files in the dataset
    async fn list_files(&self, repo_url: &str, id: &str) -> anyhow::Result<Vec<FileEntry>>;
}

struct MockFilemetrixClient {}

impl MockFilemetrixClient {
    fn new() -> Self {
        MockFilemetrixClient {}
    }
}

#[async_trait::async_trait]
impl FilemetrixClient for MockFilemetrixClient {
    async fn get_dataset_info(&self, repo_url: &str, id: &str) -> anyhow::Result<DatasetInfo> {
        let dataset_info = DatasetInfo {
            // mock all fields, they are from filemetrix API call.
            repo_url: repo_url.to_string(),
            dataset_id: id.to_string(),
            description: "example01".to_string(),
            total_files: None,
            total_size_bytes: None,
            created_at: Some(Timestamp::default()),
            updated_at: Some(Timestamp::default()),
            tags: HashMap::new(),
        };
        Ok(dataset_info)
    }

    async fn list_files(&self, repo_url: &str, id: &str) -> anyhow::Result<Vec<FileEntry>> {
        todo!()
    }
}

pub struct Packager {
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
impl DatasetService for Packager {
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
            let repo_url = &req.datarepo_url;
            let id = &req.dataset_id;

            let dataset_info = match filemetrix_client.get_dataset_info(repo_url, id).await {
                Ok(info) => info,
                Err(err) => {
                    let err = BrowseError {
                        code: ErrorCode::UnavailableFilemetrix as i32,
                        message: format!("unable to get dataset info of url: {repo_url} - id: {id}, because of filemetrix error: {err}"),
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
                event: Some(Event::Progress(req_packager::BrowseProgress {
                    files_scanned: 0,
                    bytes_scanned: 0,
                    percent: 0,
                    path: None,
                })),
            }))
            .await
            .ok();

            // Browsing, keep on sending file info of the dataset asynchronously
            let files = match filemetrix_client.list_files(repo_url, id).await {
                Ok(files) => files,
                Err(err) => {
                    let err = BrowseError {
                        code: ErrorCode::UnavailableFilemetrix as i32,
                        message: format!("unable to list files url: {repo_url} - id: {id}, because of filemetrix error: {err}"),
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
                        message: format!("unable to send file: {repo_url} - id: {id} - file: {filepath} to client, because of: {err}"),
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
                        event: Some(Event::Progress(req_packager::BrowseProgress {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and based on their are less updated, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let filemetrix_client = Arc::new(MockFilemetrixClient::new());
    let packager = Packager {
        filemetrix: filemetrix_client,
    };

    Server::builder()
        .add_service(DatasetServiceServer::new(packager))
        .serve(addr)
        .await?;
    Ok(())
}
