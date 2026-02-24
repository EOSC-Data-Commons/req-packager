use req_packager::grpc::{dataset_service_client::DatasetServiceClient, BrowseDatasetRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: 
    // - client list all the files in a dataset
    // - client select some files and ask "what vre should I use"?
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

    Ok(())
}
