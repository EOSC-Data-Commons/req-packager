// Basic Types

type SlotName = string;

// Branded type for safety (like Rust newtype)
type AuthToken = string & { readonly __brand: unique symbol };

function makeAuthToken(s: string): AuthToken {
    return s as AuthToken;
}

type JsonValue =
  | string
  | number
  | boolean
  | null
  | { [key: string]: JsonValue }
  | JsonValue[];

type RawValue = JsonValue;

interface FileEntry {
  name: string;
  path: string; // or URL / Blob depending on your system
}


// Slot Schema (ToolMeta side)

type Slot =
  | {
      id: string;
      name: SlotName;
      kind: "value";
      valueType: "string" | "number" | "boolean" | "json";
      required?: boolean;
      description?: string;
      default?: unknown;
    }
  | {
      id: string;
      name: SlotName;
      kind: "file";
      required?: boolean;
      description?: string;
      fileFormats?: string[]; // e.g. ["csv", "png"]
      multiple?: boolean;
    };


// Tool Metadata (Schema)

interface ToolMeta {
  /** EOSC registry ID */
  id: string;

  version: string;
  name: string;
  uri: string;

  /** tags / categories */
  types: string[];

  description: string;

  /** input schema */
  slots: Slot[];
}


// Slot Values (Runtime Data)

type SlotValue =
  | { kind: "value"; value: RawValue }
  | { kind: "file"; file: FileEntry };


// Launch Input (Runtime Request)

type LaunchInput =
  | { kind: "dataset"; url: string }
  | { kind: "slots"; slots: Record<SlotName, SlotValue> }
  | { kind: "files"; files: FileEntry[] }
  | {
      kind: "slots_and_files";
      slots: Record<SlotName, SlotValue>;
      files: FileEntry[];
    };


// Dispatcher Interface

interface Dispatcher {
  launch(
    uid: string,
    token: AuthToken,
    tool: ToolMeta,
    input: LaunchInput
  ): Promise<string>; // task UUID in the DB as string, so the task can be retrieved.
}
