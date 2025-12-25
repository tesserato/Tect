from pydantic import BaseModel


class Type(BaseModel):
    name: str

    def __hash__(self):
        return hash(self.name)


class data(Type):
    pass


class error(Type):
    pass


class Function(BaseModel):
    name: str
    consumes: list[Type]
    produces: list[Type]


Credentials = data(name="Credentials")
Session = data(name="Session")
UserProfile = data(name="UserProfile")
PathToConfiguration = data(name="PathToConfiguration")
InitialCommand = data(name="InitialCommand")
Settings = data(name="Settings")
SourceFile = data(name="SourceFile")
Article = data(name="Article")
SiteTemplates = data(name="SiteTemplates")
HTML = data(name="HTML")

ProcessInitialCommand = Function(
    name="ProcessInitialCommand",
    consumes=[InitialCommand],
    produces=[Settings, PathToConfiguration],
)

ReadConfiguration = Function(
    name="ReadConfiguration",
    consumes=[PathToConfiguration],
    produces=[Settings],
)

LoadTemplates = Function(
    name="LoadTemplates",
    consumes=[Settings],
    produces=[SiteTemplates],
)

ParseSource = Function(
    name="ParseSource",
    consumes=[SourceFile],
    produces=[Article],
)

RenderArticle = Function(
    name="RenderArticle",
    consumes=[Article, SiteTemplates],
    produces=[HTML],
)

WriteHTML = Function(
    name="WriteHTML",
    consumes=[HTML],
    produces=[],
)

pool = [InitialCommand]

functions = [
    ProcessInitialCommand,
    ReadConfiguration,
    LoadTemplates,
    ParseSource,
    RenderArticle,
    WriteHTML,
]

for func in functions:
    for req in func.consumes:
        if req not in pool:
            raise ValueError(f"Unsatisfied dependency: {req.name} for function {func.name}")
        else:
            pool.remove(req)
    pool.append(func.produces)
    print(pool)