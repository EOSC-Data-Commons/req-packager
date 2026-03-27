#![allow(clippy::unused_async, clippy::too_many_lines)]
use futures_util::StreamExt;
use std::{collections::HashMap, sync::OnceLock};

use axum::{
    extract::{Path, Query},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use humansize::{make_format, DECIMAL};
use jsonwebtoken::{encode, EncodingKey, Header};
use once_cell::sync::Lazy;
use req_packager::grpc::{
    self, dataset_service_client::DatasetServiceClient, tool_service_client::ToolServiceClient,
    BrowseDatasetRequest, FindToolsRequest, ToolMeta,
};
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tokio_util::io::StreamReader;
use tonic::{metadata::MetadataValue, transport::Channel, Request};
use tower_http::services::ServeDir;
use uuid::Uuid;

static DATASETS: Lazy<HashMap<Uuid, DatasetMeta>> = Lazy::new(|| {
    let mut map = HashMap::new();

    let items = vec![
        DatasetMeta {
            uuid: compute_uuid_from_string("https://example.com/datasets/0"),
            description: "(dataverse) Replication dataset for the study 'Urban Mobility Patterns in European Cities'. Includes anonymized GPS traces and processed mobility networks.".to_string(),
            source_url: "https://dataverse.harvard.edu/dataset.xhtml?persistentId=doi:10.7910/DVN/6M2OVH".to_string(),
        },
        DatasetMeta {
            uuid: compute_uuid_from_string("https://example.com/datasets/1"),
            description: "(hal) Experimental data and simulation scripts for 'Thermal transport in layered van der Waals materials'. Contains raw measurement data and analysis notebooks.".to_string(),
            source_url: "https://hal.science/hal-04234567".to_string(),
        },
        DatasetMeta {
            uuid: compute_uuid_from_string("https://example.com/datasets/2"),
            description: "(zenodo) Dataset accompanying the publication 'Benchmarking Graph Neural Networks for Molecular Property Prediction'. Includes curated molecular graphs and training splits.".to_string(),
            source_url: "https://zenodo.org/records/10456789".to_string(),
        },
    ];

    for item in items {
        map.insert(item.uuid, item);
    }

    map
});

pub fn templates() -> &'static Tera {
    static TEMPLATES: OnceLock<Tera> = OnceLock::new();
    TEMPLATES.get_or_init(|| {
        let mut tera = match Tera::new("ui/templates/**/*") {
            Ok(t) => t,
            Err(err) => {
                println!("Parsing error(s): {err}");
                ::std::process::exit(1);
            }
        };
        tera.autoescape_on(vec![".html", ".sql"]);
        tera
    })
}

pub fn get_dataset(uuid: &Uuid) -> Option<&'static DatasetMeta> {
    DATASETS.get(uuid)
}

#[derive(Serialize, Debug, Clone)]
struct FileMeta {
    download_url: Option<String>,
    data_path: String,
    filename: String,
    size: String,
    is_dir: bool,
    mimetype: Option<String>,
}

impl From<grpc::FileEntry> for FileMeta {
    fn from(value: grpc::FileEntry) -> Self {
        let formatter = make_format(DECIMAL);
        Self {
            download_url: value.download_url,
            data_path: value.path.clone(),
            filename: value.path.clone(),
            size: formatter(value.size_bytes),
            is_dir: value.is_dir,
            mimetype: value.mime_type,
        }
    }
}

impl From<FileMeta> for grpc::FileEntry {
    fn from(value: FileMeta) -> Self {
        // FIXME: I should not get it from text type send from request, better way to get the
        // original data structure??
        Self {
            download_url: value.download_url,
            path: value.data_path.clone(),
            size_bytes: 0,
            is_dir: value.is_dir,
            mime_type: value.mimetype,
            checksum_type: None,
            checksum: None,
            modified_at: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    name: String,
    role: String,
    exp: usize,
}

pub fn create_token() -> String {
    let claims = Claims {
        sub: "user123".to_string(),
        name: "Alice".to_string(),
        role: "admin".to_string(),
        exp: 1_999_999_999, // some expiration
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(b"my_secret_key"),
    )
    .unwrap()
}

async fn fetch_dataset_files(uuid: &Uuid) -> Result<Vec<FileMeta>, Box<dyn std::error::Error>> {
    let token = create_token();
    let meta_token: MetadataValue<_> = format!("Bearer {}", token).parse()?;

    let channel = Channel::from_static("http://[::1]:50051").connect().await?;

    let mut client =
        DatasetServiceClient::with_interceptor(channel, move |mut req: tonic::Request<()>| {
            req.metadata_mut()
                .insert("authorization", meta_token.clone());
            Ok(req)
        });

    let request = tonic::Request::new(BrowseDatasetRequest {
        uuid: uuid.to_string(),
        url_datarepo: "https://example.com/datasets".to_string(),
        id_dataset: "1".to_string(),
    });

    let mut stream = client.browse_dataset(request).await?.into_inner();

    let mut files = Vec::new();

    while let Some(resp) = stream.message().await? {
        if let Some(evt) = resp.event {
            match evt {
                grpc::browse_dataset_response::Event::FileEntry(entry) => {
                    files.push(entry.into());
                }
                grpc::browse_dataset_response::Event::DatasetInfo(_) => {}
                grpc::browse_dataset_response::Event::Progress(_) => {}
                grpc::browse_dataset_response::Event::Complete(_) => break,
                grpc::browse_dataset_response::Event::Error(err) => {
                    eprintln!("error: {:?}", err);
                }
            }
        }
    }

    Ok(files)
}

async fn inspect_dataset_repo(Path(uuid): Path<Uuid>) -> Html<String> {
    // XXX: file list get from calling request on filemetrix
    let files = fetch_dataset_files(&uuid).await.unwrap();
    let mut context = Context::new();
    context.insert("uuid", &uuid);
    context.insert("files", &files);
    let html = templates()
        .render("dataset/file-list.html", &context)
        .unwrap();
    Html(html)
}

async fn repo_additional() -> Html<&'static str> {
    Html(
        r#"
<ul class="tree" id="tree">
  <details open>
    <summary>
      <span class="repo" data-path="src">https://github.com/eosc/ui-example</span>
    </summary>
    <ul>
      <!-- Root folder -->
      <li>
        <details data-path="src" open>
          <summary>
            <span class="folder" data-path="src">📁 src</span>
          </summary>
          <ul>
            <!-- Nested folder -->
            <li>
              <details data-path="src/components">
                <summary>
                  <span class="folder" data-path="src/components">📁 components</span>
                </summary>
                <ul>
                  <li class="file" data-path="src/components/button.tsx">
                    <span class="name">📄</span>
                    <input type="checkbox" class="file-checkbox" data-path="src/components/button.tsx">
                    <span class="filename">Button.tsx</span>
                    <span class="meta">
                      <span class="type">tsx</span>
                      <span class="size">5 KB</span>
                    </span>
                  </li>
                  <li class="file" data-path="src/components/modal.tsx">
                    <span class="name">📄</span>
                    <input type="checkbox" class="file-checkbox" data-path="src/components/modal.tsx">
                    <span class="filename">Modal.tsx</span>
                    <span class="meta">
                      <span class="type">tsx</span>
                      <span class="size">7 KB</span>
                    </span>
                  </li>
                </ul>
              </details>
            </li>
            <!-- Files inside src -->
            <li class="file" data-path="src/main.ts">
              <span class="name">📄</span>
              <input type="checkbox" class="file-checkbox" data-path="src/main.ts">
              <span class="filename">main.ts</span>
              <span class="meta">
                <span class="type">ts</span>
                <span class="size">12 KB</span>
              </span>
            </li>
            <li class="file" data-path="src/data.csv">
              <span class="name">📄</span>
              <input type="checkbox" class="file-checkbox" data-path="src/data.csv">
              <span class="filename">data.csv</span>
              <span class="meta">
                <span class="type">csv</span>
                <span class="size">2.1 MB</span>
              </span>
            </li>
          </ul>
        </details>
      </li>
    </ul>
  </details>
</ul>
    "#,
    )
}

async fn vre_with_id(Path(id): Path<u64>) -> Html<String> {
    // XXX: from id to get the vre from tool registry
    // FIXME: css not properly set for vre description that goes very long.
    if id > 1 {
        Html(format!(
            r#"
<h3>VRE entity: {id}</h3>
<div class="vre-description">
  <p>
    Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
  </p>
</div>
<div class="launch">
  <button class="launch-btn" data-action="launch-vre">Launch</button>
</div>
<div class="vre-inputs">
  <div class="vre-slot" data-slot="input-1">
    <div class="slot-title">Input files A</div>
    <div class="slot-hint">Drop files here</div>
  </div>
  <div class="vre-slot" data-slot="input-2">
    <div class="slot-title">Input files B</div>
    <div class="slot-hint">Drop files here</div>
  </div>
  <div class="vre-slot" data-slot="input-3">
    <div class="slot-title">Input files C</div>
    <div class="slot-hint">Drop files here</div>
  </div>
</div>
    "#
        ))
    } else {
        Html(format!(
            r#"
<h3>VRE entity: {id}</h3>
<div class="vre-description">
  <p>
    Duis aute irure dolor in reprehenderit 
  </p>
</div>
<div class="launch">
  <button class="launch-btn" data-action="launch-vre">Launch</button>
</div>
<div class="vre-inputs">
  <div class="vre-slot" data-slot="input-1">
    <div class="slot-title">Input files A</div>
    <div class="slot-hint">Drop files here</div>
  </div>
  <div class="vre-slot" data-slot="input-2">
    <div class="slot-title">Input files B</div>
    <div class="slot-hint">Drop files here</div>
  </div>
</div>
    "#
        ))
    }
}

async fn preview_file(Query(params): Query<HashMap<String, String>>) -> Html<String> {
    let download_url = params.get("url").unwrap();
    let content = format!(
        "// mock preview\n// filename: I'll streaming file to tmp from {download_url} and print it here,\n\n this is only for demo purpose, depend on the mime-type I use different EOSC builtin tools to open and preview the file\n\nfn main() {{\n    println!(\"hello\");\n}}"
    );

    let title = format!(
        "on url: {}, with mime-type: {}",
        download_url,
        params.get("mimetype").unwrap_or(&"unknown".to_string())
    );

    // XXX: hx-on is not working.
    Html(format!(
        r#"
<div id="file-preview" style="display:block;">
  <div id="file-preview-header">
    <span id="file-preview-title">{title}</span>
    <button
      hx-on:click="this.closest('#file-preview').style.display='none'">
      ✖
    </button>
  </div>
  <pre id="file-preview-content">{content}</pre>
</div>
"#,
    ))
}

async fn download_file(Query(params): Query<HashMap<String, String>>) -> Response {
    // Get the remote URL
    let remote_url = match params.get("url") {
        Some(url) => url,
        None => return (axum::http::StatusCode::BAD_REQUEST, "Missing url").into_response(),
    };

    // Determine mimetype (optional, fallback to generic)
    let mimetype = params
        .get("mimetype")
        .map(|s| s.as_str())
        .unwrap_or("application/octet-stream");

    // Fetch the remote file
    dbg!(remote_url);
    let resp = match reqwest::get(remote_url).await {
        Ok(r) => r,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                "Failed to fetch remote file",
            )
                .into_response()
        }
    };

    // Map reqwest Bytes stream to Result<hyper::Chunk, std::io::Error>
    let stream = resp.bytes_stream();
    // TODO: this works well for small files, but not chunked if files are large, in EOSC, it is
    // reasonable to make this assumption.
    let body = axum::body::Body::from_stream(stream);

    // Extract filename from URL
    let filename = remote_url.split('/').next_back().unwrap_or("file.dat");

    Response::builder()
        .header(CONTENT_TYPE, mimetype)
        .header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(body)
        .unwrap()
}

#[derive(Serialize, Clone)]
pub struct DatasetMeta {
    uuid: Uuid,
    source_url: String,
    description: String,
}

fn compute_uuid_from_string(input: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, input.as_bytes())
}

async fn search_result() -> Html<String> {
    // Mocked datasets similar to results returned from Dataverse / HAL / Zenodo
    let ds_vec: Vec<&DatasetMeta> = DATASETS.values().collect();

    let mut context = Context::new();
    context.insert("datasets", &ds_vec);

    let html = templates().render("index.html", &context).unwrap();
    Html(html)
}

#[derive(Serialize)]
struct Dataset {
    metadata: DatasetMeta,
}

async fn dataset(Path(uuid): Path<Uuid>) -> Html<String> {
    let mut context = Context::new();
    match get_dataset(&uuid) {
        Some(ds) => {
            let ds = Dataset {
                metadata: ds.clone(),
            };
            context.insert("ds".to_string(), &ds);
        }
        None => {
            return Html(format!("Dataset {} not found", uuid));
        }
    };

    let html = templates().render("dataset/index.html", &context).unwrap();
    Html(html)
}

async fn find_tools(files: &[FileMeta]) -> Result<Vec<ToolMeta>, Box<dyn std::error::Error>> {
    let token = create_token();
    let meta_token: MetadataValue<_> = format!("Bearer {}", token).parse()?;

    let channel = Channel::from_static("http://[::1]:50051").connect().await?;

    let mut client =
        ToolServiceClient::with_interceptor(channel.clone(), move |mut req: Request<()>| {
            req.metadata_mut()
                .insert("authorization", meta_token.clone());
            Ok(req)
        });
    let request = tonic::Request::new(FindToolsRequest {
        files: files.iter().map(|f| f.clone().into()).collect(),
    });
    let resp = client.find_tools(request).await?.into_inner();
    let tools = resp.tools;

    Ok(tools)
}

async fn vre_recommend_from_files(Form(form): Form<HashMap<String, String>>) -> Html<String> {
    // XXX: mock behavior that:
    // 1. one file select => vre1
    // 2. two files select => vre2
    // 3. else number of files select => vre2 + vre3
    let file_list = if form.is_empty() {
        "".to_string()
    } else {
        let files = form
            .iter()
            .enumerate()
            .map(|(i, (f1, _))| format!("idx:{}, {}", i, f1))
            .collect::<Vec<_>>()
            .join(",");
        format!(" got files {}", files)
    };

    match form.len() {
        0 => Html(format!(
            r"
          <h3>Recommonded VREs:</h3>
          <div>
            <p>{file_list}</p>
            </br>
            No file selected.
          </div>
    "
        )),
        1 => Html(format!(
            r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <p>{file_list}</p>
            </br>
            <button onclick="console.log('vre 1 clicked')" type="button" hx-get="/vre/1" hx-target="#vre" hx-swap="innerHTML">vre 1</button>
          </div>
    "##
        )),
        2 => Html(format!(
            r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <p>{file_list}</p>
            </br>
            <button onclick="console.log('vre 2 clicked')" type="button" hx-get="/vre/2" hx-target="#vre" hx-swap="innerHTML">vre 2</button>
          </div>
    "##
        )),
        _ => Html(format!(
            r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <p>{file_list}</p>
            </br>
            <button onclick="console.log('vre 1 clicked')" type="button" hx-get="/vre/1" hx-target="#vre" hx-swap="innerHTML">vre 1</button>
            <button onclick="console.log('vre 2 clicked')" type="button" hx-get="/vre/2" hx-target="#vre" hx-swap="innerHTML">vre 2</button>
          </div>
    "##
        )),
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .nest_service("/assets", ServeDir::new("ui/assets"))
        .route("/search-result", get(search_result))
        .route("/", get(search_result))
        .route("/datasets/{id}", get(dataset))
        .route("/datasets/{id}/repo", get(inspect_dataset_repo))
        .route("/repo-additional", get(repo_additional))
        // preview with the download_url that `preview_file` can stream and read, with query passed
        // in indicate which mime-type it is etc.
        .route("/preview", get(preview_file))
        .route("/download", get(download_file))
        .route("/vre-recommend-from-files", post(vre_recommend_from_files))
        .route("/vre/{id}", get(vre_with_id));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
