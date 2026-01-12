use req_packager::{
    grpc::{
        assemble_service_server::AssembleServiceServer,
        dataset_service_server::DatasetServiceServer, DatasetInfo, FileEntry,
    },
    DataRepoRelayer, DispatcherClient, FilemetrixClient, InfoRequest, LaunchRequset,
    ReqPackAssembler, ToolRegistryClient,
};

use prost_types::Timestamp;
use std::{collections::HashMap, sync::Arc};
use tonic::transport::Server;
use url::Url;

use req_packager::VirtualResearchEnv;

#[derive(Debug)]
struct Dataset {
    // XXX: I don't want to couple the grpc logic with business logic, so I need real type for both
    // datasetinfo and fileentry.
    info: DatasetInfo,
    files: Vec<FileEntry>,
}

struct MockFilemetrixClient {
    datasets: HashMap<(String, String), Dataset>,
}

impl MockFilemetrixClient {
    fn new(datasets: Vec<Dataset>) -> Self {
        let datasets: HashMap<(String, String), Dataset> = datasets
            .into_iter()
            .map(|ds| {
                let info = ds.info.clone();
                let (url, id_ds) = (info.url_datarepo, info.id_dataset);
                ((url, id_ds), ds)
            })
            .collect();
        MockFilemetrixClient { datasets }
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    // XXX: when new type/tool added, do I want to reload the packager in the memory?
    // pro: tool/type-registry is more static and based on their are less updated, query is faster
    // (however there is not too much query needed, just index visiting).
    // con: the packager need to be initialized, how freq it happens to take latest list?
    //
    let filemetrix = Arc::new(MockFilemetrixClient::new(vec![]));
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
