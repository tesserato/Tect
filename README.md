# tect
A meta-programming language for reasoning about code architecture.



- [ ] Hover description in keywords (error, data, etc)
- [ ] functions are always pure, data is always immutable
- [ ] format this document
- [ ] show errors, etc: make sure when parsing is broken


I am thinking of adding a new type called group, to let the user simulate structs, oop classes, etc. 

Usage:

group Group_one
group Group_two

Group_one authRes = Authenticate(userInput) # marks authRes as belonging to Group_one

errors = Group_two{
    DatabaseOffline,
    UserNotFound
}

including group, what is the exact name of what we have?

group, function, error and data are types, right?

What about the rest?