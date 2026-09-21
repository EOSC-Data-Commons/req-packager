export type JsonPrimitive =
  | string
  | number
  | boolean
  | null;

export type JsonValue =
  | JsonPrimitive
  | JsonValue[]
  | { [key: string]: JsonValue };

/**
 * Launch request sent from request-packager to dispatcher.
 */
export interface VreLaunchRequest {
  tool: ToolMeta;
  input: LaunchInput;

  /**
   * Overrides the default runtime platform associated
   * with the resolved VRE type.
   */
  runtime_platform?: string;
}

/**
 * Tool definition consumed by the dispatcher.
 */
export interface ToolMeta {
  id: string;
  uri: string;
  name: string;
  description: string;
  version: string;

  /**
   * Tool types used by the dispatcher to resolve the VRE.
   *
   * Examples:
   *   "galaxy"
   *   "galaxy_workflow"
   *   "oscar"
   *   "binder"
   *   "egi-replay"
   *   "vip"
   *   "boutique"
   *   "scipion"
   *   "jupyter"
   *   "mddash"
   *   "sciencemesh"
   *   "cernbox"
   *   "mybinder"
   *   "binder-launcher"
   *   "rrp"
   */
  types: string[];

  slots: SlotDefinition[];

  /**
   * Original tool definition.
   *
   * The dispatcher preserves this as an opaque value in
   * the generated RO-Crate when it is non-empty.
   */
  raw_definition: JsonValue;
}

export interface SlotDefinition {
  id: string;
  name: string;
  slot_typ: SlotType;
  /**
   * Whether the slot is optional.
   */
  is_optional: boolean;
}

export type SlotType =
  | "Num"
  | "Flag"
  | "Str"
  | "File";

/**
 * User-provided launch inputs.
 *
 * The Rust implementation uses a map keyed by slot name.
 */
export interface LaunchInput {
  slots: Record<string, SlotValue>;

  /**
   * Present in the request model, although the current
   * RO-Crate serializer does not currently serialize it.
   */
  dataset?: Dataset;
}

export type SlotValue =
  | ValueSlot
  | FileSlot;

export interface ValueSlot {
  /**
   * Value used directly as FormalParameter.defaultValue.
   */
  kind: "value";
  value: JsonValue;
}

export interface FileSlot {
  /**
   * File used as a File entity and as the @id of
   * FormalParameter.defaultValue.
   */
  kind: "file";
  file: FileEntry;
}

export interface FileEntry {
  /**
   * Path/name of the file.
   *
   * Used as the RO-Crate File.name and as a fallback
   * identifier when download_url is unavailable.
   */
  path: string;
  /**
   * URL from which the file can be downloaded.
   */
  download_url?: string;
  mime_type?: string;
  size_bytes: number;

  /**
   * Checksum value.
   *
   * The current serializer emits this as `sha256`.
   * The checksum type itself is not stored in the current
   * Rust FileEntry model.
   */
  checksum?: string;
}

export interface Dataset {
  url: string;
  title: string;
  description: string;
}

// For the RO-Crate JSON-LD itself, 
// I keep a second set of types rather than trying to make VreLaunchRequest itself look like RO-Crate:

export interface RoCrate {
  "@context": "https://w3id.org/ro/crate/1.1/context";
  "@graph": RoCrateEntity[];
}

export type RoCrateEntity =
  | RoCrateMetadataDescriptor
  | RoCrateRootDataset
  | RoCrateWorkflow
  | RoCrateComputerLanguage
  | RoCrateFormalParameter
  | RoCrateFile
  | RoCrateDataset
  | RoCrateToolMetadata
  | RoCrateAuthor
  | RoCrateWorkflowHub
  | RoCrateLicense;

export interface RoCrateMetadataDescriptor {
  "@id": "ro-crate-metadata.json";
  "@type": "CreativeWork";
  about: EntityReference;
  conformsTo: EntityReference;
}

export interface RoCrateRootDataset {
  "@id": "./";
  "@type": "Dataset";
  name: string;
  description: "N/A";
  datePublished: string;
  license: EntityReference;
  creator: EntityReference;
  mainEntity: EntityReference;
  hasPart: EntityReference[];
}

export interface RoCrateWorkflow {
  "@id": string;
  "@type": (
    | "File"
    | "SoftwareSourceCode"
    | "ComputationalWorkflow"
  )[];

  conformsTo: EntityReference;
  name: string;
  description: string;
  programmingLanguage: EntityReference;
  creator: EntityReference;
  dateCreated: string;
  license: EntityReference;
  sdPublisher: EntityReference;
  version: string;
  runtimePlatform: string;

  encodingFormat?: string;
  input?: EntityReference[];
}

export interface RoCrateComputerLanguage {
  "@id": string;
  "@type": "ComputerLanguage";
  identifier: string;
  name: string;
  url: string;
}

export interface RoCrateFormalParameter {
  "@id": string;
  "@type": "FormalParameter";
  name: string;
  additionalType: SlotType;
  required: boolean;
  defaultValue?: JsonValue | EntityReference;
}

export interface RoCrateFile {
  "@id": string;
  "@type": "File";
  name: string;
  license: EntityReference;
  encodingFormat?: string;
  url?: string;
  contentSize: number;
  sha256?: string;
}

export interface RoCrateDataset {
  "@id": string;
  "@type": "Dataset";
  name: string;
  description: string;
}

export interface RoCrateToolMetadata {
  "@id": "#tool-metadata";
  "@type": "Thing";
  rawDefinition: JsonValue;
}

export interface RoCrateAuthor {
  "@id": "#author-dispatcher";
  "@type": "Person";
  name: "Dispatcher System";
}

export interface RoCrateWorkflowHub {
  "@id": "#workflow-hub";
  "@type": "Organization";
  name: "Example Workflow Hub";
  url: "http://example.com/workflows/";
}

export interface RoCrateLicense {
  "@id": "#license-unspecified";
  "@type": "CreativeWork";
  name: "Unspecified license";
  description: "License not specified by the crate producer";
}

export interface EntityReference {
  "@id": string;
}
