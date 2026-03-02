#![allow(clippy::unused_async, clippy::too_many_lines)]
use std::{collections::HashMap, sync::OnceLock};

use axum::{
    Form, Json, Router,
    extract::{Path, Query},
    response::Html,
    routing::{get, post},
};
use humansize::{DECIMAL, make_format};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};
use tower_http::services::ServeDir;

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

#[derive(Serialize)]
struct FileMeta {
    data_path: String,
    filename: String,
    size: String,
    mimetype: String,
}

async fn inspect_dataset_repo(Path(id): Path<u64>) -> Html<String> {
    let formatter = make_format(DECIMAL);
    // XXX: file list get from calling request on filemetrix
    let file_vec: Vec<FileMeta> = vec![
        FileMeta {
            data_path: "/files/main.txt".to_string(),
            filename: "main.txt".to_string(),
            size: formatter(12_000_000u64),
            mimetype: "txt".to_string(),
        },
        FileMeta {
            data_path: "/files/dummy.tar.gz".to_string(),
            filename: "dummy.tar.gz".to_string(),
            size: formatter(54_000_000u64),
            mimetype: "tar.gz".to_string(),
        },
        FileMeta {
            data_path: "/files/data.csv".to_string(),
            filename: "data.csv".to_string(),
            size: formatter(12_400u64),
            mimetype: "csv".to_string(),
        },
        FileMeta {
            data_path: "/files/report.pdf".to_string(),
            filename: "report.pdf".to_string(),
            size: formatter(123_420u64),
            mimetype: "pdf".to_string(),
        },
    ];
    let mut context = Context::new();
    context.insert("id", &id);
    context.insert("files", &file_vec);
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

#[derive(Serialize)]
struct DatasetMeta {
    id: String,
    source_url: String,
    description: String,
}

async fn search_result() -> Html<String> {
    // XXX: this list is result get from calling data-common-search.
    let ds_vec: Vec<DatasetMeta> = vec![
        DatasetMeta {
            id: "000".to_string(),
            description: "xxxx00".to_string(),
            source_url: "https://example.com/000".to_string(),
        },
        DatasetMeta {
            id: "001".to_string(),
            description: "xxxx01".to_string(),
            source_url: "https://example.com/001".to_string(),
        },
    ];
    let mut context = Context::new();
    context.insert("datasets", &ds_vec);
    let html = templates().render("index.html", &context).unwrap();
    Html(html)
}

#[derive(Serialize)]
struct Dataset {
    metadata: DatasetMeta,
}

async fn dataset(Path(id): Path<u64>) -> Html<String> {
    // XXX: dataset and its metadata is from checking filemetrix service using <id>.
    let desc = "Lorem ipsum dolor sit amet, 
        consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore 
        magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris 
        nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit 
        in voluptate velit esse cillum dolore eu fugiat nulla pariatur. 
        Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia 
        deserunt mollit anim id est laborum.";
    let meta = DatasetMeta {
        id: format!("{id}"),
        description: desc.to_string(),
        source_url: format!("https://example.com/{id}"),
    };
    let ds = Dataset { metadata: meta };
    let mut context = Context::new();
    context.insert("dataset000", &ds);
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
