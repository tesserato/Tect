
# Comments

lines starting with a hash (#) are comments

# Definitions

Define data and error types. Data can optionally be mutable. Mutable data is consumed by functions and, if needed later, must be outputed again by the functions that use it. immutable data remains in the pool until end of execution, as is not consumed. Functions can consume errors beside data, and output data and or errors. Errors unconsumed at the end of execution are fatal.

## Example

```bash
data mut InitialCommand 
data mut PathToConfig 
data SourceFile 
data mut Article 
data mut Html 
data Settings 
data Templates 
data Success

error FSError
error InitialCommandMalformedError
error FileNotFoundError
error ConfigurationMalformedError
error FileSystemError
error MetadataError
error TemplateError
```

# Tokens

Tokens are instantiated data, and besides the mutability atribute of their type, they have a cardinality of one by default. If enclosed in square brackets, they have cardinality of "collection".

# Functions

Functions can consume 0 to n types and errors, comma separated, and must output groups of data and or errors, optionally separated by pipes. The inputs of a function are matched against all existing pools, and must exist in at least one pool. All matches create branches. Each of pipe separated groups create a different branch (pool of tokens).

## Examples

```bash
function ScanFS(Settings)
    > [SourceFile]
    | [FSError]


function ProcessInitialCommand(InitialCommand)
    > Configuration
    | PathToConfiguration
    | InitialCommandMalformedError

function ReadConfiguration(PathToConfiguration)
    > Configuration
    | FileNotFoundError
    | ConfigurationMalformedError

function PrepareOutput(Settings)
    > Settings
    | FileSystemError

function DeployStaticAssets(Settings)
    > Settings
    | FileSystemError

function ExtractMetadata(SourceFile, Settings)
    > Article
    | MetadataError

function ResolveResources(Article)
    > Article
    | FileSystemError

function RenderPage(Article)
    > String
    | TemplateError

function FinalizeSite(Settings)
    > String
    | TemplateError
```

# Flows

<!-- TODO -->

## Examples

```bash
ProcessInitialCommand
ReadConfiguration


PrepareOutput
DeployStaticAssets


ExtractMetadata
ResolveResources
RenderPage


FinalizeSite
```