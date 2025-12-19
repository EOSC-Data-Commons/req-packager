# req-packager

`req-packager` is a middleware component of [EOSC Data Commons](https://www.eosc-data-commons.eu/) system. 
It act as a bridge between frontend [`matchmaker`](https://github.com/EOSC-Data-Commons/matchmaker) and [`dispatcher`](https://github.com/EOSC-Data-Commons/Dispatcher). 
It consist of a set of services that collect dataset and Virtual Research Environment (VRE) or tools information and assemble them into a package payload.
This payload is then send to the [`dispatcher`](https://github.com/EOSC-Data-Commons/Dispatcher) which is the gateway service of data players that responsible for preparing and launching the requsted VREs and tools.

`req-packager` interacts with the following components:

- [`matchmaker`](https://github.com/EOSC-Data-Commons/matchmaker), which is the frontend service that interacts with `req-packager` to update request status and rendering for end user.
- [`filemetrix`](https://github.com/Dans-labs/filemetrix), which provides detailed dataset metadata used during package construction.
- [`tool-registry`](github.com/Dans-labs/tool-registry), which supplies metadata for registred tools and VREs.
- [`type-registry`](https://github.com/ekoi/type-registry/blob/main/src/main.py), which offers information about supported and available file types.
- [`Dispatcher`](https://github.com/EOSC-Data-Commons/Dispatcher) is the final consumer of the package payload, containing all the information required to prepare and launch environments and tools.

The highlevel design notes can be found at `design.md`.

## License

All contributions must retain this attribution.

    MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

This software is developed as part of the [EOSC Data Commons](https://www.eosc-data-commons.eu/) project.


