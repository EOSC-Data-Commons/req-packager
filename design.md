# Design notes

The design notes contains rfcs and misc notes which will finally goes to the official PDF[1] and publish project page [2].
Every time after a RFC is merged, the PDF on typst need to be update accordingly.

[1]: https://typst.app/project/rohTQ0J9ibtW6gosRyDHnc
[2]: https://confluence.egi.eu/display/EOSCDATACOMMONS

## RFCs

### RFC 000: redefine the role of request packager

Look at the architechture design, the EOSC data common (EDC) system has two major parts the matchmaker and te data player.
There is a solid clear boundary in between because the matchmaker part focus on the data query while the data player focus on executing specific data with a desired tool.

At the moment, to bridge these two large sub-systems, the frontend takes most of responsibility.
This unfortunatly is not ideal because the frontend are supposed run on the users' browser and should just do minimal UI rendering.
Meanwhile, for such large project with many component, the client side rendering is quite limited, but this is off the topic.

It therefore requires a more versatile middleware to bridge the matchmaker and the data player.
This middleware is the request packager (RP) that consume the upstream information of datasets and virtual research environment (VRE) definitions and spit out a monolith definition send to dispatcher to ask for data player.

The RP need to interface with:
1. frontend: frontend ask for preparing and render UIs of further launching dedicate data player. 
<!-- 1. search backend (`data-commons-search`): it is where dataset metadata is from. -->
1. dispatcher: RP send a blob of information to dispatcher, so it knows what and how to prepare and launch a registered VREs.
1. filemetrix: the service for further request for more granual file information.

One of the roles of the request packager is to decouple the frontend and the dispatcher, since dispatcher is one of the key component that has vital responsibility to manage the VREs.
The frontend should not make too much assumptions on the dispatcher's APIs and that should be the responsibility of RP. 
Because the dispatcher has the requirement or the scalability, and because it has quite some heavy load on managing the authentication to talk to different data repository and different VRE providers,
it is the same for the `filemetrix` service which high likely will have large compute load on adapting the file metadata or to streaming the file for detail informations.
For scalabilities the communication logic should not coded in the frontend but through a middleware which is the request packager.

The frontend talk to RP, ask for 
1. files hierachy information which can get from filemetrix.
1. further instructions from user and ask for VRE settings and launching VRE.
1. RP talk to dispatcher to launch VRE and wait for ack.
1. ack comes and ack back to frontend.

### RFC 001: grpc over restapi

Through request packager (RP), the user is supposed to talk with multiple components in the system.
The gRPC is better solution than restAPI because of one major reason that the communication between RP and frontend/dispatcher are bileteral, otherwise need to pool to get update.
Same as restAPI, the gRPC is language agonostic, which fit the design that the frontend might be JS and backend are mostly in python.

The payload include the VRE and dataset information can be large (sending a ro-crate, and maybe even some light weight data depends on how VRE access data).
Therefore performance point of view we want to use gRPC to avoid delay.

### RFC 002: lazy filemetrix accessing and hierachy update

The files information in the dataset is the most important information for the RP.  
The information is not fully stored in the havested database (under discussino of WP4, if it contains than even easier for RP to process) but can be deduct through filemetrix.
In anycase, it is too expensive for the harvester to get actual file type because it requires the streaming certain part of the file.
However, this can happened in an async manner when open the dataset after click `run` button.
When the entry is viewed, a new page is open and all files are scanned to give a ready to play button and a customize button to select VRE/tool.
If file size <1M, it is automatically scanned, otherwise need to be triggered. By default showing 100 files, if there are more, display the number and require user's trigger to display all.
For every dataset, the view page always have a VRE button to open dataset as a folder in a platform like environment.

### RFC 003: multi-stage tool/vre preparing flow

It is very limit and resource demanding if all dataset is opened with a VRE.

There are following user stories for playing a data/dataset:

1. a single file `.csv` or `.cif` is launched to a light weight online tool (how those tool is expect to get the file? These are usually small files so send by streaming).
1. a single file `ipynb` is launched to a VRE.
1. the whole dataset is launched to a VRE.

For the last two cases, the VRE should be able to expecting a must have metadata file in the dataset to set the environment (like `.binder` for VRE, or `pyproject.toml` for python project), if not, the input is allowed to provide from user or !!selecting from template along with VRE provide specs.

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
The fourth phase is already on the VRE side so not too much EDC components involved.

### Misc 001: requirement and design comment on dispatcher

From use case perspective, I need some design from dispatch to interop with and requirements is written down in this paragraph.

- a sever protocol for dynamic new tool registry and auto tool scan and loading without intrusive release and deployment cycle on the dispatcher repo.

### Misc 002: requirement and design comment on frontend

From use case perspective, I need some design from frontend to have a proper interface to interact with the packager and dispatcher.

- the CSR by react is less ideal, should be a clear SSR design so that the rendering layer is slim and backend can be scale.
- security point of view, SSR is more proper for such large system as well.
- combine `view` and `run` button to have actually run VRE inside the view page.

## Ideas

Collect ideas which are in low-priority.

- Lazy scanning zip/tar file in the dataset on demand, have a sever (filemetrix??) to streaming the decompress and provide a lazy load (these may need some scalability).
