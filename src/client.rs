use req_packager::grpc::{
    browse_dataset_response, dataplayer_service_client::DataplayerServiceClient,
    dataset_service_client::DatasetServiceClient, get_artifact_response::EntryPoint,
    tool_service_client::ToolServiceClient, tool_service_server::ToolService, BrowseDatasetRequest,
    BrowseDatasetResponse, BrowseError, EoscInlineTool, FindToolsRequest, GetArtifactRequest,
    HostedTool, LaunchRequest, QueryUserRequest, UserId,
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

    // XXX: how can I here do lazy file loading and selection and trigger tool selection without
    // loading all files? now in poc, I simply pass all files from browse database to be select in
    // one go.
    let mut files = Vec::new();
    while let Some(resp) = stream.message().await? {
        println!("resp: {:?}", resp);
        let evt = resp.event.unwrap();
        match evt {
            browse_dataset_response::Event::FileEntry(entry) => {
                // dbg!(&entry);
                files.push(entry);
            }
            browse_dataset_response::Event::DatasetInfo(info) => {
                dbg!(info);
            }
            browse_dataset_response::Event::Progress(p) => {
                dbg!(p);
            }
            browse_dataset_response::Event::Complete(signal) => {
                dbg!(signal);
            }
            browse_dataset_response::Event::Error(err) => {
                eprintln!("{:?}", err);
            }
        }
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
    let request = tonic::Request::new(FindToolsRequest {
        files: files.clone(),
    });
    let resp = client.find_tools(request).await?.into_inner();
    // XXX: mocked based on number of files, > n use VRE_n, mock 5 vres and 10 dataset with
    // different number of files in the dataset randomly have number of files 1-10.
    // The more realistic mock is use the supported mime-type declared by the vres.
    let tools = resp.tools;
    dbg!(&tools);

    // depend on tool registry whether want to have all information in one table, the tool-meta
    // passed to dispatcher should in principle include all communication details in one payload.
    // then the files might not enough to be just an array but a mapping into the slots that tool
    // detail is expected.
    let mut client = DataplayerServiceClient::connect("http://[::1]:50051").await?;
    // XXX: assume that first tool is selected
    let tool = tools[1].clone();
    // XXX: files here is the files drag-drop (automatically assigned to, or guess what slots need what??) into the VRE input slots.
    // TODO: request to launch the vre
    let request = tonic::Request::new(LaunchRequest {
        tool: Some(tool),
        files: files.clone(),
    });
    let resp = client.launch(request).await?.into_inner();

    // after launch, the communication is all through the handler returned
    let h_id = resp.handler_id;
    let req = GetArtifactRequest { handler_id: h_id };
    let resp = client.get_artifact(req).await?.into_inner();
    let ep = resp.entry_point.unwrap();
    let callback_url = match ep {
        EntryPoint::EoscInline(t) => t.callback_url,
        EntryPoint::Hosted(t) => t.callback_url,
    };

    // a typical case to encorage user to stick with EOSC system is to get the return user a list
    // of tools status and a dashboard to see what they did, some summary for limit etc.
    let req = QueryUserRequest {
        user_id: Some(UserId {
            inner: "001".to_string(),
        }),
    };
    let result = client.query(req).await?.into_inner();
    let handlers = result.th;
    // print all status of these tool handlers
    for h in handlers {
        dbg!(h.state);
        dbg!(h.id);
        dbg!(h.owner);
    }

    Ok(())
}
