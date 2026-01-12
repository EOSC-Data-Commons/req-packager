use req_packager::grpc::{dataset_service_client::DatasetServiceClient, BrowseDatasetRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DatasetServiceClient::connect("http://[::1]:50051").await?;

    // made up repo url and dataset id, should be mocked for test
    let url_datarepo = "http://onedata.com".to_string();
    let id_dataset = "xxx-pid".to_string();
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
