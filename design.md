# Design notes of request package (RP)

The design notes contains RFCs and miscellaneous notes that will eventually be incorporated into the official PDF[1] and published on project page [2].
Every time an RFC is merged, the Typest PDF needs to be updated accordingly.

[1]: https://typst.app/project/rohTQ0J9ibtW6gosRyDHnc
[2]: https://confluence.egi.eu/display/EOSCDATACOMMONS

## Roadmaps

Based on the RFCs described in the following sections, the roadmap outlines the components required to construct the RP service.
The final deliverable will be a running server provided as a service, along with concrete APIs that clients can use to communicate with the server.
To keep the development process independent of other EDC components, the PoC mocks the remaining components in the implementation.
All mocked components will require more robust, production-quality implementations from a system-level perspective.

- [ ] [Component 001](#component-001): client ask for detail files information, dataset hierarchy.  

## RFCs

### RFC 000: Redefine the Role of the Request Packager

#### Motivation

The EOSC Data Commons (EDC) architecture is composed of two primary subsystems: the matchmaker, responsible for data discovery and querying, and the data player, responsible for executing operations on selected data within a Virtual Research Environment (VRE). 
These subsystems have a clear conceptual boundary.

Currently, the frontend acts as the primary bridge between the matchmaker and the data player. 
This places excessive responsibility on client-side code that runs in the user's browser, which should instead focus on UI rendering and lightweight interaction logic. 
Embedding orchestration and integration logic in the frontend limits scalability, complicates maintenance, and tightly couples the UI to backend APIs.

In addition, key backend components such as the dispatcher and filemetrix services are expected to handle significant workloads. 
The dispatcher manages authentication (now it did, is that proper?) and coordination across multiple data repositories and VRE providers, while filemetrix may perform compute-intensive tasks such as metadata adaptation and file streaming. 
Direct frontend interaction with these services increases coupling and hinders system evolution.

A dedicated middleware layer is therefore required to decouple the frontend from these backend services and to centralize orchestration logic.

#### Proposal

This RFC proposes redefining and strengthening the role of the Request Packager (RP) as a middleware component that bridges the matchmaker and the data player.

The Request Packager consumes upstream information -- dataset metadata (from filemetrix or db directly) and VRE definitions (from tool registry), and produces a consolidated request that can be understood by the dispatcher to prepare and launch a data player instance.

The RP interfaces with the following components:

- Frontend, who receives requests to prepare information needed for rendering UIs and launching a selected data player.
- Dispatcher, who receives structured requests from the RP describing how to prepare and launch a registered VRE.
- Filemetrix, who provides detailed file-level metadata and hierarchy information upon request.
- tool-registry, who provides VREs/tools capabilities declared for dealing with different type of dataset.

By introducing the RP as an explicit middleware layer, the frontend is decoupled from the dispatcher's internal APIs and operational complexity. 
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
gRPC offers native support for bidirectional streaming, making it a more suitable solution for this type of interaction.

gRPC, like REST APIs, is language-agnostic, allowing the frontend to be implemented in JavaScript while backend components remain in Python. 
This aligns with the system's design goals of language interoperability and flexibility.

gRPC is designed for streaming large payloads. 
Compared to REST APIs, it uses a binary protocol over HTTP/2, which enables more efficient data transfer and lower latency.
This opens up the possibility of handling files without requiring the dispatcher to communicate directly with all data repositories. 
Instead, the request packager can be introduced as a service to manage medium-sized files (approximately 1–100 MB) and forward them to subsequent VRE operations.

#### Proposal

To reduce latency and improve performance, gRPC should be used instead of REST APIs. 
This ensures efficient communication, supports streaming large payloads, and facilitates real-time updates between components.

### RFC 002: Lazy filemetrix accessing and hierarchy update

#### Motivation

File-level information is the most critical data for the Request Packager (RP) to prepare datasets for VRE execution. 
However, this information is not fully stored in the harvested database (maybe it can? under discussion in WP4). 
Even if the database contains partial information, retrieving the actual file types or content requires streaming parts of the files, which is expensive and inefficient to perform during harvesting.

#### Proposal

File information should be accessed lazily through filemetrix when the user interacts with the dataset, rather than pre-fetching all details. 
When a user opens a dataset after clicking run (or view), a new page is loaded. 
All files are scanned asynchronously to prepare a Ready-to-Play button and a Customize button for VRE/tool selection. 
Files smaller than 1 MB are scanned automatically, while larger files require user-triggered scanning. 
By default, only 100 files are displayed. 
If the dataset contains more, the page shows the number of additional files and allows the user to trigger loading of all files. Every dataset view page provides a VRE button to open the dataset as a folder in a platform-like environment.

### RFC 003: Multi-Stage Tool/VRE Preparing Flow

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

## Components

### Component 001

Client send dataset metadata, server use input to retrieve futher files hierarchy or file info of files in the dataset.

There are two options of getting file info in the dataset:

- More detailed file information is already havested and stored in the database.
- The information is retrieved lazily from data repository.

By storing information in the database can make display very responsive, however with following downsides:

- The file info is get in the havesting phase thus might out of sync with data repository (or if a data repository is offline temporarily).
- Extra specs requires on describing how data store in the DB and be used in the RP, which makes the development iteration slower and every change on spec requires re-havesting that is unaffordable.

The second factor is the major issue of using stored file infos because it is impossible to maintain it to keep sync of spec and keep on redo the havesting process.
We there retrieve data lazily when the dataset is viewed in the frontend.

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

### Misc 002: requirement and design comment on frontend

From use case perspective, I need some design from frontend to have a proper interface to interact with the packager and dispatcher.

- the CSR by react is less ideal, should be a clear SSR design so that the rendering layer is slim and backend can be scale.
- security point of view, SSR is more proper for such large system as well.
- combine `view` and `run` button to have actually run VRE inside the view page.
- the `view` is now redirect to the source, IMO it is better called "source", and "view" replace "run" as mentioned above.

## Ideas

Collect ideas which are in low-priority.

- Lazy scanning zip/tar file in the dataset on demand, have a sever (filemetrix??) to streaming the decompress and provide a lazy load (these may need some scalability).
