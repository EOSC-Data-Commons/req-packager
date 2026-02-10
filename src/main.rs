#![allow(clippy::unused_async, clippy::too_many_lines)]
use std::collections::HashMap;

use axum::{
    Router,
    extract::{Path, Query},
    response::Html,
    routing::get,
};
use tower_http::services::ServeDir;

async fn repo() -> Html<&'static str> {
    Html(
        r##"
<ul class="tree" id="tree">
  <li>
    <details data-path="a-repo-example" open>
      <summary>
        <span class="folder" data-path="doi:base/example/dataset">base dataset</span>
      </summary>
      <ul>
        <li class="file" data-path="src/main.ts">
          <span class="name">main.ts</span>
          <span class="meta">
            <span class="type">ts</span>
            <span class="size">12 KB</span>
          </span>
          <!-- tool button for file -->
          <button 
            class="tool-btn" 
            title="preview" 
            hx-get="/files/main.rs"
            hx-target="#file-preview"
            hx-swap="outerHTML"
            hx-vals='{"path": "__ROOT__/files/main.ts"}'>
            preview
          </button>
          <!-- dropdown button -->
          <div class="dropdown">
            <button class="dropdown-btn" data-action="toggle-dropdown">Open with ▾</button>
            <div class="dropdown-content">
              <button data-action="open-file" data-path="files/main.ts">
                Tool x
              </button>
              <button onclick="console.log('Tool 2 clicked')">Tool 2</button>
              <button onclick="console.log('Tool 3 clicked')">Tool 3</button>
            </div>
          </div>
          <button data-action="download-file" data-path="files/main.ts">
            Download
          </button>
        </li>
        <li class="file" data-path="src/dummy.tar.gz">
          <span class="name">dummy.tar.gz</span>
          <span class="meta">
            <span class="type">tar.gz</span>
            <span class="size">20 KB</span>
          </span>
          <!-- tool button for file -->
          <!-- <button  -->
          <!--   class="tool-btn"  -->
          <!--   data-path="__ROOT__/files/dummy.tar.gz"  -->
          <!--   title="preview"  -->
          <!--   hx-get="files/dummy.tar.gz" -->
          <!--   hx-target="#file-preview-content" -->
          <!--   hx-swap="innerHTML"> -->
          <!--   preview -->
          <!-- </button> -->
          <!-- dropdown button -->
          <div class="dropdown">
            <button class="dropdown-btn" data-action="toggle-dropdown">Open with ▾</button>
            <div class="dropdown-content">
              <button data-action="open-file" data-path="files/dummy.tar.gz">
                Tool x
              </button>
              <button onclick="console.log('Tool 2 clicked')">Tool 2</button>
              <button onclick="console.log('Tool 3 clicked')">Tool 3</button>
            </div>
          </div>
          <button data-action="download-file" data-path="files/dummy.tar.gz">
            Download
          </button>
        </li>
        <li class="file" data-path="src/data.csv">
          <span class="name">data.csv</span>
          <span class="meta">
            <span class="type">csv</span>
            <span class="size">2.1 MB</span>
          </span>
          <button class="tool-btn" data-path="src/data.csv" title="preview">preview</button>
        </li>
        <li class="file" data-path="src/report.pdf">
          <span class="name">report.pdf</span>
          <span class="meta">
            <span class="type">pdf</span>
            <span class="size">840 KB</span>
          </span>
          <button class="tool-btn" data-path="src/report.pdf" title="preview">preview</button>
        </li>
        <li class="file" data-path="src/index.html">
          <span class="name">index.html</span>
          <span class="meta">
            <span class="type">html</span>
            <span class="size">6 KB</span>
          </span>
          <button class="tool-btn" data-path="src/index.html" title="preview">preview</button>
        </li>
      </ul>
    </details>
  </li>
</ul>
    "##,
    )
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
    Html(format!(
        r#"
<h2>VRE {id}</h2>
<div class="vre-description">
  <p>
    Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore
    eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident,
    sunt in culpa qui officia deserunt mollit anim id est laborum.
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

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/repo", get(repo))
        .route("/repo-additional", get(repo_additional))
        .route("/files/{filename}", get(read_file))
        .route("/vre/{id}", get(vre_with_id))
        .fallback_service(ServeDir::new("ui"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
