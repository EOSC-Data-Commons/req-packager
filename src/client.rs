use req_packager::grpc::{
    assemble_service_client::AssembleServiceClient,
    dataplayer_service_client::DataplayerServiceClient,
    dataset_service_client::DatasetServiceClient, get_artifact_response::EntryPoint,
    tool_service_client::ToolServiceClient, tool_service_server::ToolService, BrowseDatasetRequest,
    BrowseDatasetResponse, EoscInlineTool, GetArtifactRequest, HostedTool, PackageAssembleRequest,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO:
    // - client list all the files in a dataset
    //  |- for every file, client go and get a lightweight tool for preview it
    // - client select some files and ask (throttle 200ms) "what vre should I use" (streaming back
    // the results)?
    // - with files and vre, create a ro-crate that contains all information inside (with
    // information of which file is in which vre input slots).
    // - client get the realtime status update from VRE and get a callback link send back when it
    // is ready.
    let mut client = DatasetServiceClient::connect("http://[::1]:50051").await?;

    // made up repo url and dataset id, should be mocked for test
    let url_datarepo = "https://example.com/datasets".to_string();
    let id_dataset = "1".to_string();
    let request = tonic::Request::new(BrowseDatasetRequest {
        url_datarepo,
        id_dataset,
    });

    let mut stream = client.browse_dataset(request).await?.into_inner();
    while let Some(resp) = stream.message().await? {
        println!("resp: {:?}", resp);
    }

    // assemble the package from 1. selected files, 2. the selected vre. 3. misc config if there
    // are some. same information used to construct the ro-crate on the client side. Use the same function (in
    // shared util, that is the request package), to construct the ro-crate in the grpc server
    // side (the one in front of dispatcher). (is the ro-crate very very important? it is actually make the interface flasky,
    // because it is not programmatically type safe).
    // The idea on ro-crate: the ro-crate will not be transfered over tcp wire but it is constructed in the
    // both end using the same function. This make the two ends can use strong type system to
    // formalize the message instead of using ro-crate which is not so easy to work with.

    let mut client = ToolServiceClient::connect("http://[::1]:50051").await?;
    // TODO: request to find tools from selected files
    let resp = client.find_tools(request).await?.into_inner();
    let tools = resp.tools;

    let mut client = DataplayerServiceClient::connect("http://[::1]:50051").await?;
    let tool = tools[1];
    let resp = client.launch(request).await?.into_inner();
    let tool_handler = resp.handler;
    if let Some(handler) = tool_handler {
        let id = handler.id;
        let req = GetArtifactRequest {
            handler_id: todo!(),
        };
        let artifact = client.get_artifact(req).await?.into_inner();
        let ep = artifact.entry_point.unwrap();
        let callback_url = match ep {
            EntryPoint::EoscInline(t) => t.callback_url,
            EntryPoint::Hosted(t) => t.callback_url,
        };
    }

    Ok(())
}
