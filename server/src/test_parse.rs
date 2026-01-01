use pest::Parser;
use pest_derive::Parser;
use std::fs;

#[derive(Parser)]
#[grammar = "tect.pest"]
pub struct TectParser;

#[test]
fn main() {
    let unparsed_file = fs::read_to_string("../samples/dsbg.tect").expect("cannot read file");

    let file = TectParser::parse(Rule::program, &unparsed_file)
        .expect("unsuccessful parse") // unwrap the parse result
        .next()
        .unwrap(); // get and unwrap the `file` rule; never fails

    for record in file.into_inner() {
        let rule = record.as_rule();
        let span = record.as_span();
        let inner = record.into_inner();
        println!("{:?}\n{:?}\n{:?}\n\n", rule, span, inner);
    }
}
