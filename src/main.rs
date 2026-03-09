#![allow(clippy::unused_async, clippy::too_many_lines)]
use std::{collections::HashMap, sync::OnceLock};

use axum::{
    extract::{Path, Query},
    response::Html,
    routing::{get, post},
    Form, Json, Router,
};
use humansize::{make_format, DECIMAL};
use jsonwebtoken::{encode, EncodingKey, Header};
use once_cell::sync::Lazy;
use req_packager::grpc::{
    self, dataset_service_client::DatasetServiceClient, BrowseDatasetRequest,
};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tonic::{metadata::MetadataValue, transport::Channel};
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

#[derive(Serialize, Debug)]
struct FileMeta {
    data_path: String,
    filename: String,
    size: String,
    mimetype: Option<String>,
}

impl From<grpc::FileEntry> for FileMeta {
    fn from(value: grpc::FileEntry) -> Self {
        let formatter = make_format(DECIMAL);
        Self {
            data_path: value.path.clone(),
            filename: value.path.clone(),
            size: formatter(value.size_bytes),
            mimetype: value.mime_type,
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
      <span class="repo" data-path="src">https//github.com/eosc/ui-example</span>
    </summary>
    <ul>
      <!-- Root folder -->
      <li>
        <details data-path="src" open>
          <summary>
            <span class="folder" data-path="src">src</span>
          </summary>
          <ul>
            <!-- Nested folder -->
            <li>
              <details data-path="src/components">
                <summary>
                  <span class="folder" data-path="src/components">components</span>
                </summary>
                <ul>
                  <li class="file" data-path="src/components/button.tsx">
                    <span class="name">Button.tsx</span>
                    <span class="meta">
                      <span class="type">tsx</span>
                      <span class="size">5 KB</span>
                    </span>
                  </li>
                  <li class="file" data-path="src/components/modal.tsx">
                    <span class="name">Modal.tsx</span>
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
              <span class="name">main.ts</span>
              <span class="meta">
                <span class="type">ts</span>
                <span class="size">12 KB</span>
              </span>
            </li>
            <li class="file" data-path="src/data.csv">
              <span class="name">data.csv</span>
              <span class="meta">
                <span class="type">csv</span>
                <span class="size">2.1 MB</span>
              </span>
            </li>
            <li class="file" data-path="src/report.pdf">
              <span class="name">report.pdf</span>
              <span class="meta">
                <span class="type">pdf</span>
                <span class="size">840 KB</span>
              </span>
            </li>
            <li class="file" data-path="src/index.html">
              <span class="name">index.html</span>
              <span class="meta">
                <span class="type">html</span>
                <span class="size">6 KB</span>
              </span>
            </li>
          </ul>
        </details>
      </li>
      <!-- Config files at root -->
      <li class="file" data-path=".gitignore">
        <span class="name">.gitignore</span>
        <span class="meta">
          <span class="type">txt</span>
          <span class="size">512 B</span>
        </span>
      </li>
      <li class="file" data-path="package.json">
        <span class="name">package.json</span>
        <span class="meta">
          <span class="type">json</span>
          <span class="size">1 KB</span>
        </span>
      </li>
      <li class="file" data-path="tsconfig.json">
        <span class="name">tsconfig.json</span>
        <span class="meta">
          <span class="type">json</span>
          <span class="size">2 KB</span>
        </span>
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
</div>
    "#
        ))
    }
}

async fn read_file(
    Path(filename): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let content = format!(
        "// mock preview\n// filename: {filename}\n\nfn main() {{\n    println!(\"hello\");\n}}"
    );

    let full_path = params.get("path").unwrap_or(&filename);

    // XXX: hx-on is not working.
    Html(format!(
        r#"
<div id="file-preview" style="display:block;">
  <div id="file-preview-header">
    <span id="file-preview-title">{full_path}</span>
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

async fn vre_recommend_from_files(Form(form): Form<HashMap<String, String>>) -> Html<String> {
    // XXX: mock behavior that:
    // 1. one file select => vre1
    // 2. two files select => vre2
    // 3. else number of files select => vre2 + vre3
    match form.len() {
        0 => Html(r"
          <h3>Recommonded VREs:</h3>
          <div>
            No file selected.
          </div>
    ".to_string()),
        1 => Html(r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <button onclick="console.log('vre 1 clicked')" type="button" hx-get="/vre/1" hx-target="#vre" hx-swap="innerHTML">vre 1</button>
          </div>
    "##.to_string()),
        2 => Html(r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <button onclick="console.log('vre 2 clicked')" type="button" hx-get="/vre/2" hx-target="#vre" hx-swap="innerHTML">vre 2</button>
          </div>
    "##.to_string()),
        _ => Html(r##"
          <h3>Recommonded VREs:</h3>
          <div>
            <button onclick="console.log('vre 1 clicked')" type="button" hx-get="/vre/1" hx-target="#vre" hx-swap="innerHTML">vre 1</button>
            <button onclick="console.log('vre 2 clicked')" type="button" hx-get="/vre/2" hx-target="#vre" hx-swap="innerHTML">vre 2</button>
          </div>
    "##.to_string()),
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
        .route("/files/{filename}", get(read_file))
        .route("/vre-recommend-from-files", post(vre_recommend_from_files))
        .route("/vre/{id}", get(vre_with_id));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
