# tect
A meta-programming language for reasoning about code architecture.



- [ ] Hover description in keywords (error, data, etc)
- [ ] functions are always pure, data is always immutable
- [ ] format this document
- [ ] show errors, etc: make sure when parsing is broken


# kinds

data
error recuperable errors : warning if not dealt with
group
function
flow?
ok? 
panic? Irrecuperable error, aborts program

# keywords
loop?
match?

# types

data Table
data pathString
data BooleanShouldSave

error FileNotFound
error 
# functions

receive errors and or data and can output multiple combinations of errors and data

function loadTable(pathString) > Table | FileNotFound
function saveTable(Table, pathString) > ok | panic


The program maintains a pool of types (multiset).

Each function:

consumes some types from the pool (its inputs)

adds some types to the pool (its outcomes)

Execution proceeds step by step.

At the end of the flow:

the pool must be empty

except for errors

Any unconsumed error is fatal.



# Example login
what about this:

data Credentials
data Session
data UserProfile

error InvalidPassword
error DatabaseOffline
error UserNotFound

function Authenticate(Credentials)
  > Session
  | InvalidPassword
  | DatabaseOffline

function FetchProfile(Session)
  > UserProfile
  | UserNotFound
  | DatabaseOffline


Authenticate # Credentials > Session | InvalidPassword | DatabaseOffline
FetchProfile {
    InvalidPassword { stop }
    UserNotFound     { stop }
    DatabaseOffline  { retry }
}

# Example DSBG

mut data InitialCommand 
mut data PathToConfig 
data SourceFile 
mut data Article 
mut data Html 
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

function ExtractMetadata(SourceFile)
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


ProcessInitialCommand
ReadConfiguration


PrepareOutput
DeployStaticAssets


ExtractMetadata
ResolveResources
RenderPage


FinalizeSite