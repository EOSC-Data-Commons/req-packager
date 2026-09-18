/// with the integration of dispatcher, all the following inner implementation can be replaced.
/// The `deprecated.rs` file here for the backup purpose. I will replace VRE by VRE.
    async fn launch(
        &self,
        user_info: &UserInfo,
        token: &RawToken,
        tool: &ToolMeta,
        input: &LaunchInput,
        api_keys: &HashMap<String, String>,
    ) -> anyhow::Result<Uuid> {
        // it also relates to the auth problem, who has the access to the vre? who should control
        // the permission of vre. I think it should be the vre provider and somewhere there is a
        // mapping for what eosc user can access which vres. Should this all kept in an auth server
        // (assume it will be one), or dispatcher maintain the table and mapping??
        // thus non-mock one should take care of auth here or somewhere in front

        // in the mock, this will be just
        // 1. have a in-memory db (mocked by HashMap) to record user and tools launched
        // 2. return a dummy url to be printed in the UI frontend.

        // should tool meta contain all info to let dispatcher know "how to launch a tool?"

        // VIP
        // RRP
        //

        // XXX: mock only the galaxy behavior, @reggie we need to find the pattern here to do the proper
        // abstraction.
        //
        // POST https://usegalaxy.eu/api/workflow_landings
        // ```json
        // {
        //   "public": false,
        //   "workflow_id": "https://dockstore.org/api/ga4gh/trs/v2/tools/%23workflow%2Fgithub.com%2Flaitanawe%2Fismb2024%2Fgalaxy_example/versions/main/PLAIN_GALAXY/descriptor/Galaxy-Workflow-reverse_file_galaxy_workflow.ga", # trs is one of the ga4gh spec, defind the API, of it is a workflow or tool.
        //   "workflow_target_type": "trs_url", # trs standard
        //   "request_state": {
        //     "simpletext_input": {
        //       "class": "File",
        //       "filetype": "txt",
        //       "location": "https://example-files.online-convert.com/document/txt/example.txt"
        //     }
        //   }
        // }
        // ```
        // HTTP 200
        // [Asserts]
        // jsonpath "$.uuid" exists
        // [Captures]
        // landing_uuid: jsonpath "$.uuid"
        //
        // return this url
        // # GET https://usegalaxy.eu/workflow_landings/{{landing_uuid}}?public=false

        // NOTE: @reggie, I want ToolMeta include info says "I am a tool need to use galaxy as VRE
        // to launch me. Then in the realworld `launch` implementation it goes to dispatcher to
        // launch the galaxy with the specific information attached."
        //
        // struct ToolMeta {
        //     // id, version, name, description, slots are needed by the UI.
        //     id: String,
        //     version: String,
        //     name: String,
        //     description: String,
        //
        //     // XXX: !!! runtime is a VRE type which indicate how the tool need to be launched.
        //     // not very clear how the VRE specific information passed to here, because different
        //     // VRE has different information required, therefore the layout of input is dynamic.
        //     // `workflow_id` and `trs_url` are such kinds of information.
        //
        //     runtime: RuntimeMeta,
        // }
        //
        // struct RuntimeMeta {
        //     kind: RuntimeKind, // this not dynamically support adding new runtime
        //     config: serde_json::Value,
        // }
        //
        // enum RuntimeKind {
        //     Galaxy,
        //     RRP,
        //     VIP,
        // }
        //
        // fun foo(tool: ToolMeta) {
        //      match tool.runtime.kind {
        //          RuntimeKind::Galaxy {
        //              let cfg: GalaxyRuntime = serde_json::from_value(runtime.config)?;
        //              ...
        //          }
        //      }
        // }
        //
        // // json will be like
        // // {
        // //   "id": "toolid-706",
        // //   "runtime": {
        // //     "config": {
        // //       "workflow_id": "xxx",
        // //       "workflow_target_type": "trs_url",
        // //       "request_state": {
        // //         "simpletext_input": {
        // //           "class": "File",
        // //           "filetype": "txt",
        // //           "location": "https://example-files.online-convert.com/document/txt/example.txt"
        // //         }
        // //       }
        // //     }
        // //   }
        // // }
        //
        // proxy/plugin runs for every VRE in its own process and talk to dispatcher with a well
        // defined protocol.

        // TODO: tool slots type need to be validated here before send the final launch action.
        // This can also happens in the frontend to prevent user pass the wrong type.
        // libmagic (its ML support version) can be used to do file type validation beyond the
        // extension.

        // NOTE: the actual logic here should be:
        // 1. check the tool type, if it is a) from workflowhub and b) galaxy tool
        // 2. get the workflowhub ga4ph link.
        // 3. assemble the payload
        // 4. send the payload
        //
        // NOTE: There are two variable approaches:
        // 1. tool meta contains only the vre id, the vre payload in assemble by the specific
        //    service.
        // 2. tool meta contains runtime type (id to identify the vre again), but contain the
        //    config with known layout for VREs.
        //
        // jyu: approach (1) is more proper in production, but require dispatcher / or another
        // component play the role as "VRE" registry.
        //
        #[derive(Deserialize)]
        struct VersionResp {
            id: String,
            name: String,
        }

        let uid = &user_info.sub;

        if tool.types.contains(&"galaxy_workflow".to_string())
            && tool.types.contains(&"workflowhub".to_string())
        {
            let client = reqwest::Client::new();
            let workflow_id = tool.uri.split('/').next_back().unwrap();
            // XXX: @reggie, I need to make an extra call to get the latest version id, because
            // what stored in your tool registry response is the tag of the version.
            let res = client
                .get(format!(
                    "https://workflowhub.eu/ga4gh/trs/v2/tools/{}/versions",
                    workflow_id
                ))
                .send()
                .await?;

            let resp_versions: Vec<VersionResp> = res.json().await?;
            // XXX: sehr ugly
            let version = resp_versions
                .into_iter()
                .filter(|i| i.name == tool.version)
                .map(|i| i.id)
                .collect::<Vec<_>>();

            // NOTE: only launch the latest version
            // TODO: in tool registry, harvest all version and in matchmaker UI allow to select
            // versions.
            let workflow_id = format!(
                "https://workflowhub.eu/ga4gh/trs/v2/tools/{}/versions/{}",
                workflow_id, version[0],
            );

            let slots = &input.slots;

            let request_state: serde_json::Map<String, serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    match entry {
                        SlotValue::Value(_) => {
                            // FIXME: get values from rpc client
                            todo!()
                        }
                        SlotValue::File(f) => {
                            let location = f.download_url.as_deref()?;

                            let filetype = f.path.rsplit('.').next().unwrap_or("txt");

                            Some((
                                key.clone(),
                                // @reggie galaxy specific info
                                serde_json::json!({
                                    "class": "File",
                                    "filetype": filetype,
                                    "location": location
                                }),
                            ))
                        }
                    }
                })
                .collect();

            let payload = serde_json::json!({
                "public": false,
                "workflow_id": workflow_id,
                "workflow_target_type": "trs_url",
                "request_state": request_state,
            });

            #[derive(serde::Deserialize)]
            struct Response {
                uuid: String,
            }

            // XXX: this is a blocking call, blocking call should not stay in async block.
            // See if galaxy provide async call that return immediately with a handler to check the
            // state.
            let res = client
                .post("https://usegalaxy.eu/api/workflow_landings")
                .json(&payload)
                .send()
                .await?;

            let data: Response = res.json().await.unwrap();
            let landing_uuid = data.uuid;
            let callback_url = Url::from_str(&format!(
                "https://usegalaxy.eu/workflow_landings/{landing_uuid}?public=false"
            ))
            .expect("a valid url");

            let id = uuid::Uuid::new_v4();
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(id).or_insert(task_handler);

            Ok(id)
        } else if tool.types.contains(&"boutique".to_string())
            && tool.types.contains(&"vip".to_string())
        {
            // VIP case

            // this will be the task handle id stored in the dispatcher DB
            // and the id is also used in VIP job as the id in the job name.
            let task_id = uuid::Uuid::new_v4();

            let user_agent = format!("eosc-coordinator/{}", env!("CARGO_PKG_VERSION"));
            let mut headers = HeaderMap::new();
            if let Some(key) = api_keys.get("vip") {
                headers.insert("apikey", HeaderValue::from_str(&key.to_string())?);
                // dbg!(key);
            }
            let client = ClientBuilder::new()
                .user_agent(user_agent)
                .default_headers(headers)
                .use_native_tls()
                .build()?;

            // NOTE: this is the payload
            //
            // POST https://vip.creatis.insa-lyon.fr/test/rest/executions
            // apikey: {{VIP_API_KEY}}
            // ```json
            // {
            //     "name" : "test-http-with-api",
            //     "pipelineIdentifier":"CQUEST/0.6",
            //     "resultsLocation" : "/vip/Home",
            //     "inputValues" : {
            //         "parameter_file": "https://www.creatis.insa-lyon.fr/~abonnet/quest_param_117T_A.txt",
            //         "data_file": "https://www.creatis.insa-lyon.fr/~abonnet/Rec003_Vox1.mrui",
            //         "zipped_folder": "https://www.creatis.insa-lyon.fr/~abonnet/basis_11_7.zip"
            //     }
            // }
            // ```

            let slots = &input.slots;

            // TODO:
            let request_state: serde_json::Map<String, serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    let slot_id = tool
                        .slots
                        .iter()
                        .find(|s| s.name == *key)
                        .map(|s| s.id.clone())?;
                    match entry {
                        SlotValue::Value(v) => Some((slot_id, v.clone())),
                        SlotValue::File(f) => {
                            // VIP use the slot id as the key of the file list in the payload
                            let location = f.download_url.as_deref()?;

                            let v = serde_json::Value::String(location.to_string());
                            Some((slot_id, v))
                        }
                    }
                })
                .collect();

            let pipe_name = format!("{}/{}", tool.name, tool.version);

            let payload = serde_json::json!({
                "name": format!("eosc-{task_id}"),
                "pipelineIdentifier": pipe_name,
                "resultsLocation": "/vip/Home",
                "inputValues": request_state,
            });

            #[derive(serde::Deserialize)]
            struct Response {
                uuid: String,
            }

            // XXX: this is a blocking call, blocking call should not stay in async block.
            // See if galaxy provide async call that return immediately with a handler to check the
            // state.
            let _resp = client
                .post("https://vip.creatis.insa-lyon.fr/test/rest/executions")
                .json(&payload)
                .send()
                .await?;

            // XXX: here should propagate the error because the payload can be wrong and the job
            // cannot be start.
            // check the resp state and return the error, or return Uuid but make it directly as
            // failed??

            // TODO: the response can be used for state tracking

            // XXX: vip is redesign the ui, thus there will be a redirect link to the launched job.
            let callback_url =
                Url::from_str("https://vip.creatis.insa-lyon.fr/home.html").expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"mybinder".to_string()) {
            let task_id = uuid::Uuid::new_v4();
            // todo: need to use the helper function i provide in datahugger to get the branch or
            // commit number.
            let callback_url = Url::from_str(&input.dataset.url).expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"binder-launcher".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            let raw = &tool.raw_definition;

            // ---- required fields ----
            let binder_base = raw
                .get("binder_base")
                .and_then(|v| v.as_str())
                .unwrap_or("https://mybinder.org")
                .trim_end_matches('/');

            let launcher_repo = raw
                .get("launcher_repo")
                .and_then(|v| v.as_str())
                .expect("missing launcher_repo");

            let launcher_ref = raw
                .get("launcher_ref")
                .and_then(|v| v.as_str())
                .unwrap_or("main");

            let target_repo = raw
                .get("target_repo")
                .and_then(|v| v.as_str())
                .expect("missing target_repo");

            // optional
            let branch = raw.get("branch").and_then(|v| v.as_str());
            let notebook_path = raw.get("notebook_path").and_then(|v| v.as_str());
            let overwrite = raw
                .get("overwrite")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let cleanup = raw
                .get("cleanup")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let run_postbuild = raw
                .get("run_postbuild")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let inner_urlpath = {
                // inner query builder
                let mut inner = form_urlencoded::Serializer::new(String::new());

                inner.append_pair("repo", target_repo);

                if let Some(branch) = branch {
                    if !branch.is_empty() && branch != "main" {
                        inner.append_pair("branch", branch);
                    }
                }

                if let Some(path) = notebook_path {
                    if !path.is_empty() {
                        inner.append_pair("notebookpath", path);
                    }
                }

                if !overwrite {
                    inner.append_pair("overwrite", "0");
                }

                if cleanup {
                    inner.append_pair("cleanup", "1");
                }

                if run_postbuild {
                    inner.append_pair("run_postbuild", "1");
                }

                // ---- env ----
                if let Some(env) = raw.get("env").and_then(|v| v.as_object()) {
                    for (k, v) in env {
                        if let Some(val) = v.as_str() {
                            inner.append_pair(k, val);
                        }
                    }
                }

                // ---- data files from input.files ----
                let mut data_files_json = Vec::new();

                let files = &input.files;
                for (name, file) in files.iter() {
                    data_files_json.push(serde_json::json!({
                        "url": file.download_url,
                        "path": name.to_string()
                    }));
                }

                let dataset_url = &input.dataset.url;
                data_files_json.push(serde_json::json!({
                    "url": dataset_url,
                    "path": null
                }));

                if !data_files_json.is_empty() {
                    let json =
                        serde_json::to_string(&data_files_json).expect("serialize data_files");

                    inner.append_pair("data", &json);
                }
                format!("launch?{}", inner.finish())
            };

            // ---- outer URL ----
            let mut callback_url = Url::parse(&format!(
                "{}/v2/gh/{}/{}",
                binder_base, launcher_repo, launcher_ref
            ))
            .expect("valid base url");

            callback_url
                .query_pairs_mut()
                .append_pair("urlpath", &inner_urlpath);

            tracing::info!("Create project: {}", callback_url);

            // ---- task handler ----
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };

            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"egi-replay".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            // construct:
            // https://replay.notebooks.egi.eu/v2/gh/EOSC-Data-Commons/binder-python-tool/v0.1.1?urlpath=notebooks/python.ipynb?dataset_url=https://zenodo.org/records/20844503
            let replay_index = "https://replay.notebooks.egi.eu/v2/gh";
            // let tool_name = "EOSC-Data-Commons/binder-python-tool";
            let tool_name = &tool.name;
            // let version = "v0.1.1";
            let version = &tool.version;
            // let urlpath = "notebooks/python.ipynb";
            let urlpath = tool
                .raw_definition
                .get("urlpath")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");
            let urlpath = urlpath.to_string();
            // let dataset_url = "https://zenodo.org/records/20844503";
            let dataset_url = &input.dataset.url;

            let callback_url = format!(
                "{replay_index}/{tool_name}/{version}?urlpath={urlpath}?dataset_url={dataset_url}"
            );

            let callback_url = Url::from_str(&callback_url).expect("a valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"cernbox".to_string()) {
            let task_id = uuid::Uuid::new_v4();

            let files = &input.files;
            let slots = &input.slots;

            let domain = "eosc.cernbox.cern.ch";
            let client = Client::builder().build()?;
            let Some(share_with) = slots.get("Shared With").map(|v| match v {
                SlotValue::File(_) => unreachable!("must be a value"),
                SlotValue::Value(v) => {
                    let serde_json::Value::String(v) = v else {
                        unreachable!("must be a string")
                    };
                    v
                }
            }) else {
                unreachable!("'Share With' must be set")
            };
            let share_with = format!("{share_with}@{domain}");

            // XXX: look at all fields here
            // ??, should name and description customized by user?
            let owner = &user_info.email;
            let email = &user_info.email;

            // TODO: (jyu) 'name' and 'preferred_username' is optinal, should I implement fallback logic?
            // let sender_display_name = &user_info.preferred_username;
            let sender_display_name = email.split('@').collect::<Vec<_>>()[0];

            // TODO: this needs to be constructed, and this is the main OCM trick.
            let sender = format!("{email}@eosc-coordinator.ethz.ch");

            fn create_rocrate(
                files: &HashMap<RenameName, FileEntry>,
                share_with: &str,
                domain: &str,
                title: &str,
            ) -> serde_json::Value {
                let mut graph: Vec<serde_json::Value> = Vec::new();
                let mut has_part: Vec<serde_json::Value> = Vec::new();

                // Take only first two files
                for (i, (name, file)) in files.iter().enumerate() {
                    let id = format!("#file-{}", i);

                    has_part.push(json!({ "@id": id }));

                    graph.push(json!({
                        "@id": id,
                        "@type": "File",
                        // XXX: this should be rename_to
                        "name": name.to_string(),
                        // "description": file.description, // ?? need this??
                        "encodingFormat": file.mime_type,
                        "url": &file.download_url
                    }));
                }

                // Root dataset
                graph.insert(0, json!({
                    "@id": "./",
                    "@type": "Dataset",
                    "name": title,
                    "description": "(yet not passed) A research data package with Jupyter notebook and datasets for sharing through ScienceMesh federation",
                    "datePublished": chrono::Utc::now().to_rfc3339(),
                    "creator": { "@id": "#creator" },
                    "runsOn": { "@id": "#destination" },
                    "hasPart": has_part
                }));

                // Metadata descriptor
                graph.push(json!({
                    "@id": "ro-crate-metadata.json",
                    "@type": "CreativeWork",
                    "about": { "@id": "./" },
                    "conformsTo": { "@id": "https://w3id.org/ro/crate/1.1" }
                }));

                // Static entities (keep your existing ones)
                graph.push(json!({
                    "@id": "#destination",
                    "@type": "Service",
                    "name": "ScienceMesh Service",
                    "url": format!("https://{domain}"),
                }));

                // XXX: redundant information, record twice.
                // The ro-crate format required by the cernbox is a subset of EDC ro-crate.
                graph.push(json!({
                    "@id": "#creator",
                    "@type": "Person",
                    "name": "TBD",
                    "userid": "TBD",
                }));

                graph.push(json!({
                    "@id": "#sender",
                    "@type": "Person",
                    "name": "TBD",
                    "userid": "TBD",
                }));

                graph.push(json!({
                    "@id": "#receiver",
                    "@type": "Person",
                    "userid": share_with,
                }));

                json!({
                    "@context": "https://w3id.org/ro/crate/1.1/context",
                    "@graph": graph
                })
            }

            let dataset_title = &input.dataset.title;
            let rocrate = create_rocrate(files, &share_with, domain, dataset_title);

            // ---- CREATE PROJECT ----
            let project_data = serde_json::json!({
                "shareWith": share_with,
                "name": dataset_title,
                // XXX: (jyu) not passed from launch tool call from upstream matchmaker
                "description": "",
                "providerId": &uuid::Uuid::new_v4(),
                "resourceId": task_id,
                "owner": owner,
                "senderDisplayName": sender_display_name,
                "sender": sender,
                "resourceType": "ro-crate",
                "shareType": "user",
                "protocol": {
                  "name": "multi",
                  "embedded": {"payload": rocrate}}
                }
            );

            println!("{}", serde_json::to_string_pretty(&project_data).unwrap());

            let api_url = format!("https://{domain}/ocm/shares");
            let resp = client.post(api_url).json(&project_data).send().await?;

            if !resp.status().is_success() {
                dbg!(&resp);
                dbg!("fail here");
                let artifact = Artifact::FailedTool;
                let task_handler = TaskHandler {
                    id: HandlerId(task_id),
                    user_id: UserId(uid.to_string()),
                    state: ToolState::Exception,
                    artifact,
                };

                let mut db = self.db.write().await;
                db.entry(task_id).or_insert(task_handler);

                return Ok(task_id);
            }

            let callback_url = Url::from_str(&format!("https://{domain}")).expect("valid url");

            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else if tool.types.contains(&"rrp".to_string()) {
            let task_id = uuid::Uuid::new_v4();
            let backend_url = "https://rrp-eosc.ethz.ch";
            let client = Client::builder().build()?;

            // let LaunchInput::FilesOnly(files) = input else {
            //     panic!("not possible.")
            // };
            let hdataset = &input.dataset;
            let doi = hdataset.url.trim_start_matches("https://doi.org/");
            // dbg!(doi);
            // doi = "10.5281/zenodo.20507550"

            // CREATE PROJECT
            let slots = &input.slots;
            let data_mounts: Vec<serde_json::Value> = slots
                .iter()
                .filter_map(|(key, entry)| {
                    let slot = tool.slots.iter().find(|s| s.name == *key)?;

                    match entry {
                        SlotValue::File(f) => {
                            let path = &f.path.trim_start_matches("__ROOT__/");

                            Some(serde_json::json!({
                                "mountPath": slot.id,
                                "source": {
                                    "type": "zenodo",
                                    "doi": doi
                                },
                                "path": path
                            }))
                        }
                        _ => None, // ignore non-files
                    }
                })
                .collect();

            let image = tool
                .raw_definition
                .get("docker_image")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");

            // FIXME: the image should coming from tool-metadata
            // FIXME: the descriptino should be the tool+dataset
            let project_data = serde_json::json!({
                "image": image,
                "name": format!("eosc-{task_id}"),
                "description": "Created via Coordinator",
                "resources": {
                    "cpu": 1.0,
                    "memMb": 2048
                },
                "dataMounts": data_mounts,
            });

            // // FIXME: start project and pass files into it
            // let Ok(oidc_agent_token) = std::env::var("OIDC_AGENT_TOKEN") else {
            //     panic!("oidc_agent_token not found in env var")
            // };
            let resp = client
                .post(format!("{}/external/dispatcher/v1/projects", backend_url))
                .bearer_auth(token)
                .json(&project_data)
                .send()
                .await?;

            if !resp.status().is_success() {
                dbg!("fail here");
                let artifact = Artifact::FailedTool;
                // TODO: use TaskHandler::new()
                let task_handler = TaskHandler {
                    id: HandlerId(task_id),
                    user_id: UserId(uid.to_string()),
                    state: ToolState::Exception,
                    artifact,
                };

                let mut db = self.db.write().await;
                db.entry(task_id).or_insert(task_handler);

                return Ok(task_id);
            }

            let location = resp
                .headers()
                .get("Location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            // NOTE: a rust excersize, why this moves??
            // let location = resp.headers().get("Location").and_then(|v| v.to_str().ok());

            tracing::info!("Create project: {}", resp.status());

            // Poll and wait until creationStatus == "Ready"
            let project_url = resp.json::<serde_json::Value>().await?["id"]
                .as_str()
                .expect("missing project id")
                .to_string();

            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project did not reach 'Ready' status after 20 attempts"
                    ));
                }

                attempts += 1;

                let status_resp = client.get(&project_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = status_resp.json().await?;
                let status = json["creationStatus"].as_str().unwrap_or("");

                if status == "Ready" {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait creation: attempts {}", attempts);
            }

            let repository_url = tool
                .raw_definition
                .get("repositoryUrl")
                .and_then(|v| v.as_str())
                .expect("didn't find urlpath");

            // Clone repository (async)
            let clone_resp = client
                .post(format!("{}/clone", project_url))
                .bearer_auth(token)
                .json(&serde_json::json!({
                    "repositoryUrl": repository_url
                }))
                .send()
                .await?;

            let clone_json: serde_json::Value = clone_resp.json().await?;

            // Poll clone execution until "Success"
            let execution_url = clone_json["execution"]
                .as_str()
                .expect("missing execution url")
                .to_string();

            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project did not reach 'Ready' status after 20 attempts"
                    ));
                }

                attempts += 1;

                let resp = client.get(&execution_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = resp.json().await?;
                let status = json["status"].as_str().unwrap_or("");

                if status == "Success" {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait execution: attempts {}", attempts);
            }

            // Checkout branch (blocking)
            client
                .post(format!("{}/checkout", project_url))
                .bearer_auth(token)
                .json(&serde_json::json!({
                    "ref": "main"
                }))
                .send()
                .await?
                .error_for_status()?;

            // Trigger data retrival
            client
                .post(format!("{}/data", project_url))
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?;

            // poll and wait for
            let mut attempts = 0;
            loop {
                if attempts >= 20 {
                    return Err(anyhow::anyhow!(
                        "Project (file staging) did not reach 'Ready' status after 20 attempts"
                    ));
                }
                attempts += 1;

                let resp = client.get(&project_url).bearer_auth(token).send().await?;

                let json: serde_json::Value = resp.json().await?;

                let is_all_slot_staged = slots.iter().all(|(key, _)| {
                    let slot = tool
                        .slots
                        .iter()
                        .find(|&s| s.name == *key)
                        // TODO: (jyu) This should be an error to tool developer (and who
                        // registered the tool), not to user. (But user can report to tool provider).
                        .expect("slot id should align between tool meta and input");
                    let s = json["dataStatus"][&slot.id]["status"]
                        .as_str()
                        .unwrap_or("");
                    s == "Available"
                });

                if is_all_slot_staged {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(2)).await;

                tracing::info!("Poll and wait datastaging: attempts {}", attempts);
            }

            // return the callback url

            let project_code = location
                .and_then(|loc| loc.split('/').next_back().map(|s| s.to_owned()))
                .expect("project id not there");

            tracing::info!("Project code: {}", project_code);

            let callback_url = Url::from_str(&format!("{}/projects/{}", backend_url, project_code))
                .expect("valid url");
            tracing::info!("Callback URL: {}", callback_url);

            // XXX: get status should be moved to monitor_state.
            // the state monitor is a dummy one that directly send Ready signal.
            // It should be send a stream with updating states.

            ///// -----------
            let artifact = Artifact::HostedTool {
                callback: callback_url,
            };
            // TODO: use TaskHandler::new()
            let task_handler = TaskHandler {
                id: HandlerId(task_id),
                user_id: UserId(uid.to_string()),
                state: ToolState::Ready,
                artifact,
            };

            let mut db = self.db.write().await;
            db.entry(task_id).or_insert(task_handler);

            Ok(task_id)
        } else {
            panic!("unknown support VRE");
        }
    }
