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

data Settings
data SourceFile
data Article
data SiteTemplates

error FileSystemError
error MetadataError
error TemplateError

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


PrepareOutput # Settings > Settings | FileSystemError
DeployStaticAssets

finite loop {
  ExtractMetadata
  ResolveResources
  RenderPage
}

FinalizeSite