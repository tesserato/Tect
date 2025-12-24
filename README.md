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

function loadTable(pathString) -> Table | FileNotFound
function saveTable(Table, pathString) -> ok | panic