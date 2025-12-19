pub mod req_packager {
    tonic::include_proto!("req_packager.v1");
}

use prost_types::Timestamp;
use std::{collections::HashMap, pin::Pin};
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, Stream};

use req_packager::{
    browse_dataset_response::{BrowsePhase, Event},
    dataset_service_server::{DatasetService, DatasetServiceServer},
    BrowseComplete, BrowseDatasetRequest, BrowseDatasetResponse, DatasetInfo,
};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
pub struct Packager {
    // TODO: source of tool-registry, mocked by a JSON, in production can be just tool-registry
    // API call address.
    // TODO: source of type-registry, mocked by a JSON
    // TODO: source of data repositories, mocked by a sqlite, the arch here not clear, should this
    // all behind the filemetrix? Or get from filemetrix (seems better because I don't want RP
    // tangled directly with DB, it is good to have operations behind filemetrix and this is one of
    // the roles filemetrix need to play) the basic info and query from DB after?
}

// XXX: the logic and transport mixed here, I need to have a DatasetBrowser for the inner browse
// logic, then I can do the same no matter for filemetrix, or self directy service, or mocked test.
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

        tokio::spawn(async move {
            // INIT Phase
            let req = request.get_ref();
            let repo_url = &req.datarepo_url;
            let id = &req.dataset_id;

            // TODO: make an API call using url + id (or just PID based on the API of filemetrix) to the filemetrix
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
            tx.send(Ok(BrowseDatasetResponse {
                phase: BrowsePhase::PhaseInit as i32,
                event: Some(Event::DatasetInfo(dataset_info)),
            }))
            .await
            .ok();

            // TODO: Browsing

            // COMPLETED
            tx.send(Ok(BrowseDatasetResponse {
                phase: BrowsePhase::PhaseCompleted as i32,
                event: Some(Event::Complete(BrowseComplete {
                    total_files: 100,
                    total_size_bytes: 100,
                    success: true,
                    finish_at: None,
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
    let packager = Packager::default();
    Server::builder()
        .add_service(DatasetServiceServer::new(packager))
        .serve(addr)
        .await?;
    Ok(())
}
