pub mod req_packager {
    tonic::include_proto!("req_packager.v1");
}

use req_packager::{dataset_service_client::DatasetServiceClient, BrowseDatasetRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DatasetServiceClient::connect("http://[::1]:50051").await?;

    // made up repo url and dataset id, should be mocked for test
    let datarepo_url = "http://onedata.com".to_string();
    let dataset_id = "xxx-pid".to_string();
    let request = tonic::Request::new(BrowseDatasetRequest {
        datarepo_url,
        dataset_id,
    });

    let mut stream = client.browse_dataset(request).await?.into_inner();
    while let Some(resp) = stream.message().await? {
        println!("resp: {:?}", resp);
    }

    Ok(())
}
