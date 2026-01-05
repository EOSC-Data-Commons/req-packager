# Design notes of request package (RP)

The design notes contains RFCs and miscellaneous notes that will eventually be incorporated into the official PDF[1] and published on project page [2].
Every time an RFC is merged, the Typest PDF needs to be updated accordingly.
The design here is supposed (try best) to comform with more broad package spec written in [3][4] (both of them still have lots open questions to discuss, see the comments there).

[1]: https://typst.app/project/rohTQ0J9ibtW6gosRyDHnc
[2]: https://confluence.egi.eu/display/EOSCDATACOMMONS
[3]: https://docs.google.com/document/d/1I2Z_87dYCLflf7LmJnkFWB9Y4oa6Esdv8vifhqyp-n8/edit?usp=sharing
[4]: https://confluence.egi.eu/display/EOSCDATACOMMONS/Packager

## Roadmaps

Based on the RFCs described in the following sections, the roadmap outlines the components required to construct the RP service.
The final deliverable will be a running server provided as a service, along with concrete APIs that clients can use to communicate with the server.
To keep the development process independent of other EDC components, the PoC mocks the remaining components in the implementation.
All mocked components will require more robust, production-quality implementations from a system-level perspective.

- [ ] [Component 001](#component-001): client ask for detail files information, dataset hierarchy.  

## RFCs

### RFC 000: Redefine the Role of the Request Packager

#### Motivation

The EOSC Data Commons (EDC) architecture is composed of two primary subsystems: the `matchmaker`, responsible for data discovery and querying, and the `dataplayer`, responsible for executing operations on selected data within a Virtual Research Environment (VRE). 
These subsystems have a clear conceptual boundary.

Currently, the frontend acts as the primary bridge between the matchmaker and the data player. 
This places excessive responsibility on client-side code that runs in the user's browser, which should instead focus on UI rendering and lightweight interaction logic. 
Embedding orchestration and integration logic in the frontend limits scalability, complicates maintenance, and tightly couples the UI to backend APIs.

In addition, key backend components such as the dispatcher and filemetrix services are expected to handle significant workloads. 
The dispatcher manages its own authentication (now it did, is that proper?) and coordination across multiple data repositories and VRE providers, while filemetrix (matchmaker talk to filemetrix's APIs) may perform compute-intensive tasks such as metadata adaptation and file streaming. 
Direct frontend interaction with these services increases coupling and hinders system evolution.

A dedicated middleware layer is therefore required to decouple the frontend from these backend services and to centralize orchestration logic.

#### Proposal

This RFC proposes redefining and strengthening the role of the Request Packager (RP) as a middleware component that bridges the matchmaker and the data player.

The Request Packager consumes upstream information -- dataset metadata (from filemetrix or db directly) and VRE definitions (from tool registry), and produces a consolidated request that can be understood by the dispatcher to prepare and launch a data player instance.

The RP interfaces with the following components:

- Frontend, who receives requests to prepare information needed for rendering UIs and launching a selected data player.
- Dispatcher, who receives structured requests from the RP describing how to prepare and launch a registered VRE.
- Filemetrix, who provides detailed file-level metadata and hierarchy information upon request, behind the filemetrix there is also type-registry which give a concrete type hint for a file.
- tool-registry, who provides VREs/tools capabilities declared for dealing with different type of dataset.

By introducing the RP as an explicit middleware layer, the frontend is decoupled from the dispatcher's internal APIs and operational complexity. 
The request packager assemble the metadata package incrementally and send to the dispatcher for VRE.
It avoids the package (formated as as ro-crate payload no matter in which spec), the data stream goes from RP to frontend and then from frontend to dispatcher.
All communication, orchestration, and adaptation logic required for scalability and interoperability is centralized in the RP.

The proposed interaction flow is:

- The frontend requests file hierarchy information from the RP.
- The RP retrieves the required information from filemetrix, add available (filtered by tool capabilities) tool infos and returns it to the frontend for futher user inputs.
- Based on user input, the dispatcher requests VRE instance accourding to configuration inputs and launch VREs/tools.
- The RP sends a consolidated request to the dispatcher to launch the VRE.
- When VRE is ready the dispatcher returns an acknowledgment to the RP, which forwards the redirect link to the frontend.

This approach improves separation of concerns, reduces frontend complexity, and provides a scalable and maintainable integration point between core EDC subsystems.

### RFC 001: gRPC over REST API

#### Motivation

The Request Packager (RP) needs to communicate with multiple components in the system, including the frontend and the dispatcher. 
Communication is often bidirectional, which would require polling if implemented using a traditional REST API. 
gRPC offers native support for bidirectional streaming (or at least server-to-client), making it a more suitable solution for this type of interaction.

gRPC, like REST APIs, is language-agnostic, allowing the frontend to be implemented in JavaScript while backend components remain in Python. 
This aligns with the system's design goals of language interoperability and flexibility.

gRPC is designed for streaming large payloads. 
Compared to REST APIs, it uses a binary protocol over HTTP/2, which enables more efficient data transfer and lower latency.
This opens up the possibility of handling files without requiring the dispatcher to communicate directly with all data repositories. 
Instead, the request packager can be introduced as a service to manage medium-sized files (approximately 1–100 MB) and forward them to subsequent VRE operations.

gRPC don't have head-of-line (HOL) blocking (in the HTTP level for different streams).
It is therefore much suitable for incremental client-server communication which is one of requirements in the [API design note](https://confluence.egi.eu/display/EOSCDATACOMMONS/Packager) by Wim.
With the possibility that we want small file can be directly stream to the EOSC and expose to user to open in the lightweight tools, this no HOL blocking is a must to have feature. 
Moreover, when it comes to enable user to provide required files by streaming to the target service where the file size might be large.

#### Proposal

To reduce latency and improve performance, gRPC should be used instead of REST APIs. 
This ensures efficient communication, supports streaming large payloads, and facilitates real-time updates between components.

### RFC 002: Lazy filemetrix accessing and hierarchy update

#### Motivation

File-level information is the most critical data for the Request Packager (RP) to prepare datasets for VRE execution. 
However, this information is not fully stored in the harvested database (maybe it can? under discussion in WP4). 
Even if the database contains partial information, retrieving the actual file types or content requires streaming parts of the files, which is expensive and inefficient to perform during harvesting.
The requst packager requires only very little information of the dataset (url), and perform a further lazy files hierarchy inspection when needed.
This makes the files information that processes later by user always up to date and more importantly avoid relying on the not yet finalized spec on what info to store for every dataset.

#### Proposal

File information should be accessed lazily through filemetrix when the user interacts with the dataset, rather than pre-fetching all details. 
When a user opens a dataset after clicking run (or view), a new page is loaded. 
All files are scanned asynchronously to prepare a Ready-to-Play button and a Customize button for VRE/tool selection. 
Files smaller than 1 MB are scanned automatically (the mime-type usually is unknow in the file info entry, because it is too expensive for filemetrix to get mime type by scanning), while larger files require user-triggered scanning. 
By default, only 100 files are displayed. 
If the dataset contains more, the page shows the number of additional files and allows the user to trigger loading of all files. 
This requires a bilateral rpc call that from client side pagination request can keep on sending.
Every dataset's view page provides a VRE button to open the dataset as a folder in a platform-like (e.g. RRP, Galaxy) environments.

If the `req_packager` need to take care of the download and store the file, it worth to also considering how files are stored and cached.
This is based on how most data repository provide file entries information, usually filemetrix is not able to get all those information completely.
If I need to download large files for scanning and deducting the mime-type or scanning the compressed file for hierarchy, the file need to be either in the memory or somewhere in the filesystem of the server.

There are two cases.
a) the mime-type is known in the data repository by filemetrix, the returned file entry info contain the mime-type and `req_packager` can use directly. 
In this case, the download button trigger the api that relay the file transfer from filemetrix to client and the tools are deduct from mime-type.
The relay can save the storage but might be not have consistent implementation as below case, so for prototype the file is download and stored anyway.
b) the mime-type is unknown, and the file is known to be very small (<10k). 
In this case, the file is automatically transfered from data repository to `req_packager` server and stored in the `/tmp/` (is this good?) and scanned to get/validate the checksum and mime-type.
c) the mime-type is unknow, but the file is quite large.
In this case, it is client's option to trigger the downloading and scanning, before that it only contains the filename but no further operations available before scanning.

### RFC 003: Multi-stage tool/VRE preparing flow

#### Motivation

Opening an entire dataset in a VRE for every operation is resource-intensive and often unnecessary. 
Users may want to interact with individual files or subsets of a dataset in different ways. 
To optimize resource usage and improve user experience, the system needs a multi-stage preparation flow for tools and VREs.

#### Proposal

The user interaction scenarios can be categorized into three main cases. 

- In the first case, a single file such as a `.csv` or `.cif` is launched to a lightweight online tool. These files are usually small and can be transmitted by streaming to the tool. 
- In the second case, a single file such as a Jupyter notebook (`.ipynb`) is launched in a VRE. 
- In the third case, the entire dataset is launched in a VRE.

For the last two cases, the VRE expects a mandatory metadata file in the dataset to configure the environment, similar to `.binder` files for VREs or `pyproject.toml` for Python projects. 
If such a file is missing, the system should allow the user to provide the necessary metadata manually or select it from predefined templates based on the VRE specifications.

The multi-stage preparation flow should allow for incremental setup and validation. 
Initially, lightweight tools can be launched quickly for single-file operations. 
Subsequently, metadata verification and environment preparation occur for full dataset or notebook launches. 
This staged approach balances efficiency, responsiveness, and flexibility, ensuring that the VREs are only fully instantiated when necessary and with proper configuration.

There are two divergence for how VRE allocate resources:

1. VRE provide resources (CPU/GPU) themselves, this in relatively easy from ECD point of view because after dispatcher deligate the launch signal it become all VRE's responsibility to handle the further works.
2. VRE callback EOSC for resources. It was mentioned that egi's resources should at certain point be able to be integrated and to be used by the WP7 partners. This require description on such VRE and on type of resources they can use.
3. VRE callback other tool in the registry for resources. This might be out of scope but can be a useful case that tools not only the tool for data processing but can be resource tools that anounce to owning and providing computational resources. 

### RFC 004: incremental client-side configuration with server side payload assembly

#### Motivation

This RFC proposes a design for a gRPC based configuration service where the client incrementally provides configuration information, and the server performs validation and assembles the full payload for downstream consumption. 
The goal is to optimize user experience while keeping server logic manageable and minimizing unnecessary client-side complexity.

Currently, we have two approaches to collecting configuration data for generating RO-Crate payloads:

Client-side full assembly: 

The client collects all configuration information, assembles it into a structure (e.g., `HashMap`), and sends it to the server in a single request.

- Pros: Fewer RPC calls, simpler server logic.
- Cons: Client bears full memory of user inputs, parsing complexity, delayed feedback on validation errors.

Server-side incremental assembly: 
The client sends partial configuration updates via multiple RPC calls; the server validates and updates its internal state after each call.

- Pros: Incremental validation, immediate feedback, thin client.
- Cons: Server must maintain per-client state, more RPC calls, increased complexity for session management and consistency.

#### Proposal

We use a hybrid solution, that is in the client side adding more logic to interactive getting input to construct a data structure of VRE description.
The client side dealing with basic inputs validation on the required fields and their types. 
The client side validation is localized in parameter correlation, because we would assume user may give inputs in random orders, and it is hard to validate the relation interactively.
On the server side the full constructed object is send to be validated.
The server side validation can also validate the relation of inputs.

The object of VRE description (term "metadata" is used in other EDC proposals) is a serializable data structure that can send to the server side over TCP wire.
It contains all the information required to describe how a VRE is prepared, which data should be attached and what resources can be use.
The object is cross validated on the server side before assemble to a JSON payload (ro-crate). 

To cover the different tool/VRE types described in the [RFC 003](#RFC-003-Multi-stage-tool-VRE-preparing-flow) the object (named as `VirtualResearchEnv`) need to be an enum type include subtypes:
- `EoscInline`: tool that opened inline in the page, these tool are provided by the EOSC infra for inspect single file. (out of scope, but in my opinion, easy to implement and useful).
- `BrowserNative`: tool that redirect to 3rd-party site with the selected files (therefore a proper authorization is needed), such tools are usually lightweight that using users local resource (JS/WASM) and do not need to specify resources.
- `Hosted`: VRE that need VM resources and already have resources attached by the VRE provider (e.g. RRP, Galaxy, AiiDAlab).
- `HostedWithPluginRes`: (placeholder, use case not yet clear) similar to `Hosted` but the tool is flexible to use resources provided from resource provider (cloud with credential etc.). Or such VRE type can run platform with their own resource but need extra resource from resource provider (e.g. if RRP can request for a HPC resource specified). This also partially fit requirement of "Haddock3" use case.
- `HostedWithBuiltInRes`: (placeholder, use case not yet clear) similar to `HostedWithPluginRes`, but by default use EOSC builtin resources.

For two phases validation, the dadicate validation service is required with a specification on how validator is provided by tool provider when registering tool.
This description needs to be in a human-writable format, because it is supposed to be provided by who registering the tool/VRE. 
It requires field description and rule set with an expression support DSL to describe the relation cross context. 
For the validator description, it deserve a dedicate RFC on it, see RFC005 on the requirement for this validator definition.

### RFC 005: Declarative client/server validation specification

placeholder, basic ideas, 
- need schema for fields with type validation, need rule-set for cross fields validation using common expression language (CEL).
- the basic format is yaml, which is more writable than JSON.
- the documentation with lots of complete examples are essential for such helper DSL.
- the description do not need to specify and separates client and server, it is automatically have locallized field validation on client and cross-fields validation on server.

## Components (functional requirements)

### Component 001

Look at: https://github.com/EOSC-Data-Commons/req-packager/pull/2/commits/52ec8cf0b011353e1e8e9086511c7417a3d14352.
This part of rfc will be moved to the corresponded PR.

Client send dataset metadata, server use input to retrieve futher files hierarchy or info of files in the dataset.

There are two options of getting file info in the dataset:

- More detailed file information is already havested and stored in the database.
- The information is retrieved lazily from data repository.

By storing all file metadata information in the database can make display very responsive, however with following downsides:

- The file info is get in the havesting phase thus might out of sync with data repository (or if a data repository is offline temporarily).
- Extra specs requires on describing how data store in the DB and be used in the RP, which makes the development iteration slower and every change on spec requires re-havesting that is unaffordable.

The second factor is the major issue of using stored file infos because it is impossible to maintain it to keep sync of spec and keep on redo the havesting process.
We therefore retrieve data lazily when the dataset is viewed in the frontend.

#### Proto definition

The `DatasetService` handle requests from client to further interact with data repositories.
The `BrowseDataset` call send request with data repository url and dataset id, as response it returns 

```protobuf
syntax = "proto3";

package req_packager.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

service DatasetService {
  // Lazily retrieve file hierarchy or file info for a dataset
  rpc BrowseDataset(BrowseDatasetRequest)
      returns (stream BrowseDatasetResponse);
}

message BrowseDatasetRequest {
  // Data repo url (opaque to client) 
  string datarepo_url = 1;

  // Dataset identifier (opaque to client)
  string dataset_id = 2;
}

message DatasetResponse {
  oneof event {
    DatasetInfo dataset_info = 1;
    FileEntry file_entry = 2;
    BrowseProgress progress = 3;
    BrowseError error = 4;
    BrowseComplete complete = 5;
  }
}
```

(rfc)

`FileEntry` is similar to unix file socket that can reflect both file and folder. 
In the context of scientific data repository usually the dataset are "flatten" with files.
But this not prevent from having a virtual hierarchy such as HDF4/NetCDF/Zip format has internal easy to access hierarchy for quick file accessing.
The `FileEntry` type match this idea to tell the client "this is a virtual folder, I am not showing the inner files at the moment so open me if you want to check inner hierarchy."

## Non-functional requirements

### Scalability

One of a major assumptions we made for the scalability that influence other designs is that are two part of services in the RP, one is for coordination, and the other is for temporary file storing.
The coordinate service provided by RP are mostly lightweight message broker operations, thus scale poorly.
The temporary file storing on the other hands require horizontal scaling (~10M per opened dataset).

This assumpotion makes it possible that the communication part of RP can be just one unix process to handles ~10,1000 / per second requests.
RP sit in the middle of multiple components of the EOSC system, thus any delay in the communication might become the bottleneck.
Therefore, RP does not connect to the dataset DB but do it behind filemetrix, because of DB operations are usually not very cheap.

The file storing service (see the corresponding section.xx) need to scale with number of opened datasets. 
Or ideally, if the filemetrix component is good enough to take the responsibility of this task, then the RP can be with just a pure coordination service without the need to scale.

## Miscellanous

### Misc 000: broad user behavior life cycles

The major workflow of a user to interact with the system can separated into three major phase:

1. search for a dataset 
2. inspect the dataset
3. select and request for the data player
4. play the dataset (but this is already on the player's side)

The first phase involve the frontend and the data-commons-search.
The second phase involve the frontend and the filemetrix.
The third phase invole the frontend and the request packager (underhood require the request packager talk to dispatcher and filemetrix).
The fourth phase is already on the VRE side so no more EDC components involved. (in WP7 meeting it mention the use cases of putting data back to data repositories, need to evaluate the feasibility.)

### Misc 001: requirement and design comment on dispatcher

From use case perspective, I need some design from dispatch to interop with and requirements is written down in this paragraph.

- a sever protocol for dynamic new tool registry and auto tool scan and loading without intrusive release and deployment cycle on the dispatcher repo.
- if the dispatcher is stick with restAPI, the server push with a callback may required, or the long polling (user experience and performance is not good).
- user may ask for many VREs at the same time, therefore the preparing progress information need to be record/updated in dispatch and able to be displayed back to frontend. 
- VRE cancellation.

### Misc 002: requirement and design comment on frontend

From use case perspective, I need some design from frontend to have a proper interface to interact with the packager and dispatcher.

- the CSR by react is less ideal, should be a clear SSR design so that the rendering layer is slim and backend can be scale.
- security point of view, SSR is more proper for such large system as well.
- combine `view` and `run` button to have actually run VRE inside the view page.
- the `view` is now redirect to the source, IMO it is better called "source", and "view" replace "run" as mentioned above.

### Misc 003: requirement and design commend on file-type/tool registries

The requst packager getting available tool information from tool registry and available type information from type registry. 
These two services are supposed manage by one org (cyfronet now, which is good). 
When new entries arrived, they need to cross validate if the tool's claim are i.e. meet with all the types.

## Ideas

Collect ideas which are in low-priority.

- Lazy scanning zip/tar file in the dataset on demand, have a sever (filemetrix??) to streaming the decompress and provide a lazy load (these may need some scalability).

