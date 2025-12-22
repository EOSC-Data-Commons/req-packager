use std::path::PathBuf;

// FIXME: look at EC2 etc, to have a better list of required fields
#[derive(Debug)]
struct EnvResource {
    num_cpu: u32,
    num_ram: u64,
}

/// Config for how to launch the VRE, these are specifically for e.g. `.binder`.
/// The resource description is independent of this config.
/// The request packager do not (should not??, but if tool-registry also strong typed, maybe I can
/// constructed the type easily here??) know the exact format of the config. The format is
/// encoded in the tool-registry and know b
/// TODO: if the overall architecture and tech stack can not change (ask Enol whether he want to
/// uptake the grpc in more broad scope in dispacher and tool-registry). Otherwise, check if 
/// RO-crate can provide such level of schema check.
#[derive(Debug)]
struct Config {
    inner: serde_json::Value,
}

#[derive(Debug)]
enum VirtualResearchEnv {
    // tool that opened inline in the page.
    EoscInline {
        tool_id: String,
        file: PathBuf,
    },

    // tool that redirect to 3rd-party site with the selected files
    // such tools are very lightweight and do not need to specify resources.
    BrowserNative {
        tool_id: String,
        files: Vec<PathBuf>,
    },

    // tool that need VM resources and have resources attached (e.g. RRP, Galaxy)
    Hosted {
        tool_id: String,
        config: Option<Config>,
        files: Vec<PathBuf>,
    },

    // (planned):
    // Hosted but allow to allocating using EOSC resources.
    HostedWithBuiltInRes {
        tool_id: String,
        config: Option<Config>,
        files: Vec<PathBuf>,
        res: EnvResource,
    },

    // (planned):
    // Hosted but allow to asking for tools that provide resourecs.
    HostedWithPluginRes {
        tool_id: String,
        config: Option<Config>,
        res_id: String,
        files: Vec<PathBuf>,
        res: EnvResource,
    },
}

// TODO: have a protobuf defined for the VirtualResearchEnv and mapping conversion here
//
// impl From<proto::VirtualResearchEnv> for VirtualResearchEnv {
//     fn from(value: proto::VirtualResearchEnv) -> Self {
//         match value {
//             => 
//             => 
//             => 
//             => 
//         }
//     }
// }

// server side call this function to assemble a payload that can send to downstream dispacher 
// XXX: the return type is a very generic json, I probably want a crate to handle ro-crate
// specificly.
fn assemble_vre_request(vre: &VirtualResearchEnv) -> serde_json::Value {
    match vre {
        VirtualResearchEnv::EoscInline { tool_id, file } => todo!(),
        VirtualResearchEnv::BrowserNative { tool_id, files } => todo!(),
        VirtualResearchEnv::Hosted { tool_id, config, files } => todo!(),
        VirtualResearchEnv::HostedWithBuiltInRes { tool_id, config, files, res } => todo!(),
        VirtualResearchEnv::HostedWithPluginRes { tool_id, config, res_id, files, res } => todo!(),
    }
}
